//! ffprobe and ffmpeg, and what counts as a video file.

use std::path::Path;
use std::process::Command;

use crate::encoding::encoded_dir::try_decode_name;
use crate::models::video_frame::VideoInfo;

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "webm", "mov", "mpg", "mpeg"];

pub fn new_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Whether a physical file name is a video, looking through `.dat_` encoding.
pub fn is_video_name(physical_name: &str) -> bool {
    let name = try_decode_name(physical_name).unwrap_or_else(|| physical_name.to_string());
    match name.rsplit_once('.') {
        Some((_, ext)) => VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

/// Runs ffprobe on a file. `file_size_bytes` is left at 0; the caller knows the size.
pub fn probe(path: &Path) -> Result<VideoInfo, String> {
    let output = new_command("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration,bit_rate",
            "-show_entries", "stream=width,height,display_aspect_ratio,codec_name,r_frame_rate",
            "-select_streams", "v:0",
            "-of", "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr.trim()));
    }

    parse_probe_json(&output.stdout)
}

fn parse_probe_json(stdout: &[u8]) -> Result<VideoInfo, String> {
    let json: serde_json::Value = serde_json::from_slice(stdout)
        .map_err(|e| format!("Failed to parse ffprobe JSON: {}", e))?;

    let format = &json["format"];
    let stream = json["streams"].as_array().and_then(|s| s.first());

    let duration_secs = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let bitrate = format["bit_rate"].as_str().and_then(|s| s.parse::<u64>().ok());
    let width = stream.and_then(|s| s["width"].as_u64()).map(|v| v as u32);
    let height = stream.and_then(|s| s["height"].as_u64()).map(|v| v as u32);
    let display_aspect_ratio = stream
        .and_then(|s| s["display_aspect_ratio"].as_str())
        .filter(|s| *s != "0:1")
        .map(|s| s.to_string());
    let codec = stream.and_then(|s| s["codec_name"].as_str()).map(|s| s.to_string());
    let framerate = stream
        .and_then(|s| s["r_frame_rate"].as_str())
        .map(simplify_framerate);

    Ok(VideoInfo {
        duration_secs,
        file_size_bytes: 0,
        width,
        height,
        display_aspect_ratio,
        codec,
        bitrate,
        framerate,
    })
}

/// Extracts one JPEG frame at `timestamp_secs`.
pub fn extract_frame(path: &Path, timestamp_secs: f64) -> Result<Vec<u8>, String> {
    let output = new_command("ffmpeg")
        .args(["-ss", &timestamp_secs.to_string(), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "image2pipe", "-vcodec", "mjpeg", "pipe:1"])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed at {}s: {}", timestamp_secs, stderr.trim()));
    }
    if output.stdout.is_empty() {
        return Err(format!("ffmpeg produced no output at {}s", timestamp_secs));
    }
    Ok(output.stdout)
}

pub fn simplify_framerate(rate: &str) -> String {
    if let Some((num, den)) = rate.split_once('/') {
        if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
            if d > 0.0 {
                return format!("{:.2}", n / d);
            }
        }
    }
    rate.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::encoded_dir::encode_name;

    #[test]
    fn test_is_video_name_plain() {
        assert!(is_video_name("movie.MP4"));
        assert!(is_video_name("a.b.mkv"));
        assert!(!is_video_name("notes.txt"));
        assert!(!is_video_name("mp4"));
    }

    #[test]
    fn test_is_video_name_encoded() {
        assert!(is_video_name(&encode_name("Holiday.mov")));
        assert!(!is_video_name(&encode_name("Holiday.txt")));
        assert!(!is_video_name(".dat_!!!"));
    }

    #[test]
    fn test_parse_probe_json() {
        let json = br#"{"streams":[{"codec_name":"h264","width":1920,"height":1080,
            "display_aspect_ratio":"16:9","r_frame_rate":"30000/1001"}],
            "format":{"duration":"120.5","bit_rate":"4000000"}}"#;
        let info = parse_probe_json(json).unwrap();
        assert_eq!(info.duration_secs, 120.5);
        assert_eq!(info.width, Some(1920));
        assert_eq!(info.codec.as_deref(), Some("h264"));
        assert_eq!(info.framerate.as_deref(), Some("29.97"));
        assert_eq!(info.bitrate, Some(4_000_000));
    }
}
