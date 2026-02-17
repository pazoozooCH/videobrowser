use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::cache::{self, CacheState};
use crate::encoding::encoding::decode_string;
use crate::models::video_frame::{VideoFrame, VideoInfo};

fn new_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "webm", "mov", "mpg", "mpeg"];

#[tauri::command]
pub fn list_video_files(path: String) -> Result<Vec<String>, String> {
    let dir_path = Path::new(&path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let mut results = Vec::new();
    collect_video_files(dir_path, &mut results)?;
    results.sort();
    Ok(results)
}

fn collect_video_files(dir: &Path, results: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_video_files(&path, results)?;
        } else if is_video_file(&path) {
            results.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

fn is_video_file(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return false,
    };

    // Check if it's a .dat_ encoded file
    if file_name.starts_with(".dat_") {
        let encoded_part = &file_name[5..];
        if let Some(decoded) = decode_string(encoded_part) {
            let ext = decoded.rsplit('.').next().unwrap_or("").to_lowercase();
            return VIDEO_EXTENSIONS.contains(&ext.as_str());
        }
        return false;
    }

    let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    VIDEO_EXTENSIONS.contains(&ext.as_str())
}

#[tauri::command]
pub async fn get_video_info(path: String) -> Result<VideoInfo, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let file_size_bytes = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?
        .len();

    let path_clone = path.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        new_command("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration,bit_rate",
                "-show_entries", "stream=width,height,display_aspect_ratio,codec_name,r_frame_rate",
                "-select_streams", "v:0",
                "-of", "json",
                &path_clone,
            ])
            .output()
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe JSON: {}", e))?;

    let format = &json["format"];
    let stream = json["streams"].as_array().and_then(|s| s.first());

    let duration_secs = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let bitrate = format["bit_rate"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok());

    let width = stream.and_then(|s| s["width"].as_u64()).map(|v| v as u32);
    let height = stream.and_then(|s| s["height"].as_u64()).map(|v| v as u32);
    let display_aspect_ratio = stream
        .and_then(|s| s["display_aspect_ratio"].as_str())
        .filter(|s| *s != "0:1")
        .map(|s| s.to_string());
    let codec = stream.and_then(|s| s["codec_name"].as_str()).map(|s| s.to_string());
    let framerate = stream
        .and_then(|s| s["r_frame_rate"].as_str())
        .map(|s| simplify_framerate(s));

    Ok(VideoInfo {
        duration_secs,
        file_size_bytes,
        width,
        height,
        display_aspect_ratio,
        codec,
        bitrate,
        framerate,
    })
}

fn simplify_framerate(rate: &str) -> String {
    if let Some((num, den)) = rate.split_once('/') {
        if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
            if d > 0.0 {
                let fps = n / d;
                return format!("{:.2}", fps);
            }
        }
    }
    rate.to_string()
}

fn modified_epoch_secs(path: &str) -> Result<u64, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    let modified = metadata
        .modified()
        .map_err(|e| format!("Failed to read mtime: {}", e))?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs())
}

#[tauri::command]
pub async fn extract_video_frame(
    path: String,
    timestamp_secs: f64,
    index: u32,
    cache_state: tauri::State<'_, CacheState>,
) -> Result<VideoFrame, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let modified_secs = modified_epoch_secs(&path)?;
    let modified_str = modified_secs.to_string();

    // Check cache (brief lock)
    {
        let conn = cache_state.0.lock().map_err(|e| format!("Cache lock error: {}", e))?;
        if let Some(jpeg_data) = cache::get_cached_frame(&conn, &path, &modified_str, timestamp_secs) {
            return Ok(VideoFrame {
                index,
                timestamp_secs,
                data_base64: STANDARD.encode(&jpeg_data),
            });
        }
    }

    // Run ffmpeg off the main thread
    let path_clone = path.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        new_command("ffmpeg")
            .args([
                "-ss", &timestamp_secs.to_string(),
                "-i", &path_clone,
                "-frames:v", "1",
                "-f", "image2pipe",
                "-vcodec", "mjpeg",
                "pipe:1",
            ])
            .output()
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed at {}s: {}", timestamp_secs, stderr));
    }

    if output.stdout.is_empty() {
        return Err(format!("ffmpeg produced no output at {}s", timestamp_secs));
    }

    // Store in cache (brief lock)
    {
        let conn = cache_state.0.lock().map_err(|e| format!("Cache lock error: {}", e))?;
        cache::store_frame(&conn, &path, &modified_str, timestamp_secs, &output.stdout);
    }

    Ok(VideoFrame {
        index,
        timestamp_secs,
        data_base64: STANDARD.encode(&output.stdout),
    })
}

#[cfg(test)]
fn calculate_timestamps(duration: f64, mode_type: &str, count: Option<u32>, minutes: Option<f64>) -> Result<Vec<f64>, String> {
    match mode_type {
        "fixed" => {
            let count = count.ok_or("Missing 'count' for fixed mode")?;
            if count == 0 {
                return Ok(vec![]);
            }
            let step = duration / (count as f64 + 1.0);
            Ok((1..=count).map(|i| step * i as f64).collect())
        }
        "interval" => {
            let minutes = minutes.ok_or("Missing 'minutes' for interval mode")?;
            let interval = minutes * 60.0;
            if interval <= 0.0 {
                return Err("Interval must be positive".to_string());
            }
            let mut timestamps = Vec::new();
            let mut t = interval;
            while t < duration {
                timestamps.push(t);
                t += interval;
            }
            Ok(timestamps)
        }
        _ => Err(format!("Unknown mode type: {}", mode_type)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_mode_timestamps() {
        let ts = calculate_timestamps(100.0, "fixed", Some(9), None).unwrap();
        assert_eq!(ts.len(), 9);
        assert!((ts[0] - 10.0).abs() < 0.01);
        assert!((ts[4] - 50.0).abs() < 0.01);
        assert!((ts[8] - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_fixed_mode_zero_count() {
        let ts = calculate_timestamps(100.0, "fixed", Some(0), None).unwrap();
        assert!(ts.is_empty());
    }

    #[test]
    fn test_interval_mode_timestamps() {
        // 10 minute video, frame every 2 minutes => at 120, 240, 360, 480
        let ts = calculate_timestamps(600.0, "interval", None, Some(2.0)).unwrap();
        assert_eq!(ts.len(), 4);
        assert!((ts[0] - 120.0).abs() < 0.01);
        assert!((ts[1] - 240.0).abs() < 0.01);
        assert!((ts[2] - 360.0).abs() < 0.01);
        assert!((ts[3] - 480.0).abs() < 0.01);
    }

    #[test]
    fn test_interval_mode_short_video() {
        // 30 second video, frame every 1 minute => no frames
        let ts = calculate_timestamps(30.0, "interval", None, Some(1.0)).unwrap();
        assert!(ts.is_empty());
    }

    #[test]
    fn test_unknown_mode() {
        let result = calculate_timestamps(100.0, "unknown", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_fixed_mode_missing_count() {
        let result = calculate_timestamps(100.0, "fixed", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_interval_mode_missing_minutes() {
        let result = calculate_timestamps(100.0, "interval", None, None);
        assert!(result.is_err());
    }
}
