use std::fs;
use std::path::Path;

use crate::encoding::encoded_dir::try_decode_name;
use crate::models::file_entry::FileEntry;

const EXCLUDED_FILES: &[&str] = &[".gitignore"];

#[tauri::command]
pub fn read_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir_path = Path::new(&path);

    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let entries = fs::read_dir(dir_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut file_entries: Vec<FileEntry> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        if EXCLUDED_FILES.contains(&file_name.as_str()) {
            continue;
        }

        let entry_path = entry.path();
        let is_directory = entry_path.is_dir();
        let decoded_name = try_decode_name(&file_name);
        let is_encoded = decoded_name.is_some();
        let display_name = decoded_name.clone().unwrap_or_else(|| file_name.clone());

        let has_children = if is_directory {
            fs::read_dir(&entry_path)
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
        } else {
            false
        };

        file_entries.push(FileEntry {
            path: entry_path.to_string_lossy().to_string(),
            name: display_name,
            physical_name: file_name,
            is_directory,
            is_encoded,
            decoded_name,
            has_children,
        });
    }

    file_entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(file_entries)
}
