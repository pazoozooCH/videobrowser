use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Absolute path on disk
    pub path: String,
    /// Display name (decoded if encoded, otherwise physical name)
    pub name: String,
    /// Actual filename on disk
    pub physical_name: String,
    pub is_directory: bool,
    /// Whether the file uses `.dat_` encoding
    pub is_encoded: bool,
    /// The decoded name if encoded, None otherwise
    pub decoded_name: Option<String>,
    /// Whether this directory has children (for lazy loading)
    pub has_children: bool,
}
