use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrame {
    pub index: u32,
    pub timestamp_secs: f64,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub duration_secs: f64,
    pub file_size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub display_aspect_ratio: Option<String>,
    pub codec: Option<String>,
    pub bitrate: Option<u64>,
    pub framerate: Option<String>,
}

/// What the preview of one video needs: its info, the timestamps of the mode, the frames the
/// cache already has, and the indices still to extract.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub info: VideoInfo,
    pub timestamps: Vec<f64>,
    pub frames: Vec<VideoFrame>,
    pub missing: Vec<u32>,
}
