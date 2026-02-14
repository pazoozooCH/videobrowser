use std::fs;
use std::path::Path;

use crate::encoding::encoded_dir::{can_encode, encode_name, try_decode_name};
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

#[tauri::command]
pub fn encode_node(path: String) -> Result<FileEntry, String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let physical_name = node_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?;

    let is_encoded = try_decode_name(physical_name).is_some();

    // Encode this node if not already encoded
    let current_path = if !is_encoded {
        let encoded_name = encode_name(physical_name);
        let parent = node_path.parent().ok_or("No parent directory")?;
        let new_path = parent.join(&encoded_name);
        fs::rename(node_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;
        new_path
    } else {
        node_path.to_path_buf()
    };

    // Recursively encode children if directory
    if current_path.is_dir() {
        encode_children_recursive(&current_path)?;
    }

    build_file_entry(&current_path)
}

fn encode_children_recursive(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if try_decode_name(&name).is_none() {
            let encoded_name = encode_name(&name);
            let new_path = dir.join(&encoded_name);
            fs::rename(&entry_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;

            if new_path.is_dir() {
                encode_children_recursive(&new_path)?;
            }
        } else if entry_path.is_dir() {
            encode_children_recursive(&entry_path)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn decode_node(path: String) -> Result<FileEntry, String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let physical_name = node_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?;

    let is_encoded = try_decode_name(physical_name).is_some();

    // Decode this node if encoded
    let current_path = if is_encoded {
        let decoded_name = try_decode_name(physical_name).unwrap();
        let parent = node_path.parent().ok_or("No parent directory")?;
        let new_path = parent.join(&decoded_name);
        fs::rename(node_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;
        new_path
    } else {
        node_path.to_path_buf()
    };

    // Recursively decode children if directory
    if current_path.is_dir() {
        decode_children_recursive(&current_path)?;
    }

    build_file_entry(&current_path)
}

fn decode_children_recursive(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if let Some(decoded_name) = try_decode_name(&name) {
            let new_path = dir.join(&decoded_name);
            fs::rename(&entry_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;

            if new_path.is_dir() {
                decode_children_recursive(&new_path)?;
            }
        } else if entry_path.is_dir() {
            decode_children_recursive(&entry_path)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn can_encode_node(path: String) -> Result<bool, String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    Ok(can_encode(node_path))
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy to clipboard: {}", e))
}

#[tauri::command]
pub fn rename_node(path: String, new_name: String, encode: bool) -> Result<FileEntry, String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let parent = node_path.parent().ok_or("No parent directory")?;

    let target_name = if encode {
        encode_name(&new_name)
    } else {
        new_name
    };

    let new_path = parent.join(&target_name);
    fs::rename(node_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;

    build_file_entry(&new_path)
}

fn build_file_entry(path: &Path) -> Result<FileEntry, String> {
    let physical_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let is_directory = path.is_dir();
    let decoded_name = try_decode_name(&physical_name);
    let is_encoded = decoded_name.is_some();
    let display_name = decoded_name.clone().unwrap_or_else(|| physical_name.clone());
    let has_children = if is_directory {
        fs::read_dir(path)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false)
    } else {
        false
    };

    Ok(FileEntry {
        path: path.to_string_lossy().to_string(),
        name: display_name,
        physical_name,
        is_directory,
        is_encoded,
        decoded_name,
        has_children,
    })
}
