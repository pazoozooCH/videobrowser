use std::path::Path;
use std::process::Command;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::models::video_frame::VideoFrame;

#[tauri::command]
pub fn get_video_duration(path: String) -> Result<f64, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            &path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse duration '{}': {}", stdout.trim(), e))
}

#[tauri::command]
pub fn extract_video_frame(path: String, timestamp_secs: f64, index: u32) -> Result<VideoFrame, String> {
    let file_path = Path::new(&path);
    if !file_path.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let output = Command::new("ffmpeg")
        .args([
            "-ss", &timestamp_secs.to_string(),
            "-i", &path,
            "-frames:v", "1",
            "-f", "image2pipe",
            "-vcodec", "mjpeg",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed at {}s: {}", timestamp_secs, stderr));
    }

    if output.stdout.is_empty() {
        return Err(format!("ffmpeg produced no output at {}s", timestamp_secs));
    }

    Ok(VideoFrame {
        index,
        timestamp_secs,
        data_base64: STANDARD.encode(&output.stdout),
    })
}

pub fn calculate_timestamps(duration: f64, mode_type: &str, count: Option<u32>, minutes: Option<f64>) -> Result<Vec<f64>, String> {
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
