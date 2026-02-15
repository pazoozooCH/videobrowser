use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrame {
    pub index: u32,
    pub timestamp_secs: f64,
    pub data_base64: String,
}
