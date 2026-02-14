use std::fs;
use std::path::Path;
use std::process::Command;

use crate::encoding::encoded_dir::{can_encode, encode_name, try_decode_name};
use crate::models::file_entry::FileEntry;

#[tauri::command]
pub fn get_cli_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|arg| Path::new(arg).is_dir())
}

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

        file_entries.push(FileEntry {
            path: entry_path.to_string_lossy().to_string(),
            name: display_name,
            physical_name: file_name,
            is_directory,
            is_encoded,
            decoded_name,
            has_children: is_directory,
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

#[tauri::command]
pub fn delete_node(path: String) -> Result<(), String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    if node_path.is_dir() {
        fs::remove_dir_all(node_path).map_err(|e| format!("Failed to delete directory: {}", e))
    } else {
        fs::remove_file(node_path).map_err(|e| format!("Failed to delete file: {}", e))
    }
}

#[tauri::command]
pub fn count_children(path: String) -> Result<usize, String> {
    let dir_path = Path::new(&path);
    if !dir_path.is_dir() {
        return Ok(0);
    }
    let count = fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .count();
    Ok(count)
}

#[tauri::command]
pub fn move_node(source: String, target_dir: String) -> Result<FileEntry, String> {
    let source_path = Path::new(&source);
    let target_path = Path::new(&target_dir);

    if !source_path.exists() {
        return Err(format!("Source does not exist: {}", source));
    }
    if !target_path.is_dir() {
        return Err(format!("Target is not a directory: {}", target_dir));
    }

    let file_name = source_path
        .file_name()
        .ok_or("Invalid source file name")?;
    let new_path = target_path.join(file_name);

    if new_path.exists() {
        return Err(format!("Target already exists: {}", new_path.display()));
    }

    fs::rename(source_path, &new_path).map_err(|e| format!("Failed to move: {}", e))?;

    build_file_entry(&new_path)
}

#[tauri::command]
pub fn show_in_file_manager(path: String) -> Result<(), String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let dir = if node_path.is_dir() {
        &path
    } else {
        node_path
            .parent()
            .and_then(|p| p.to_str())
            .ok_or("No parent directory")?
    };

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        if node_path.is_dir() {
            Command::new("explorer")
                .arg(dir)
                .spawn()
                .map_err(|e| format!("Failed to open explorer: {}", e))?;
        } else {
            Command::new("explorer")
                .args(["/select,", &path])
                .spawn()
                .map_err(|e| format!("Failed to open explorer: {}", e))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn open_in_vlc(path: String) -> Result<(), String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("vlc")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open VLC: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("C:/Program Files/VideoLAN/VLC/vlc.exe")
            .arg(format!("file:///{}", path.replace('\\', "/")))
            .spawn()
            .map_err(|e| format!("Failed to open VLC: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn search_files(path: String, pattern: String) -> Result<Vec<FileEntry>, String> {
    let dir_path = Path::new(&path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let pattern_lower = pattern.to_lowercase();
    let mut results = Vec::new();
    search_recursive(dir_path, &pattern_lower, &mut results)?;
    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

fn search_recursive(dir: &Path, pattern: &str, results: &mut Vec<FileEntry>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let file_name = entry.file_name().to_string_lossy().to_string();

        if EXCLUDED_FILES.contains(&file_name.as_str()) {
            continue;
        }

        let entry_path = entry.path();
        let display_name = try_decode_name(&file_name).unwrap_or_else(|| file_name.clone());

        if display_name.to_lowercase().contains(pattern) {
            results.push(build_file_entry(&entry_path)?);
        }

        if entry_path.is_dir() {
            search_recursive(&entry_path, pattern, results)?;
        }
    }

    Ok(())
}

pub fn build_file_entry(path: &Path) -> Result<FileEntry, String> {
    let physical_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let is_directory = path.is_dir();
    let decoded_name = try_decode_name(&physical_name);
    let is_encoded = decoded_name.is_some();
    let display_name = decoded_name.clone().unwrap_or_else(|| physical_name.clone());
    Ok(FileEntry {
        path: path.to_string_lossy().to_string(),
        name: display_name,
        physical_name,
        is_directory,
        is_encoded,
        decoded_name,
        has_children: is_directory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // --- build_file_entry ---

    #[test]
    fn test_build_file_entry_plain_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join("hello.txt");
        fs::write(&file, "content").unwrap();

        let entry = build_file_entry(&file).unwrap();
        assert_eq!(entry.name, "hello.txt");
        assert_eq!(entry.physical_name, "hello.txt");
        assert!(!entry.is_directory);
        assert!(!entry.is_encoded);
        assert_eq!(entry.decoded_name, None);
        assert!(!entry.has_children);
    }

    #[test]
    fn test_build_file_entry_encoded_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join(".dat_VGVzdA==");
        fs::write(&file, "content").unwrap();

        let entry = build_file_entry(&file).unwrap();
        assert_eq!(entry.name, "Test");
        assert_eq!(entry.physical_name, ".dat_VGVzdA==");
        assert!(entry.is_encoded);
        assert_eq!(entry.decoded_name, Some("Test".to_string()));
    }

    #[test]
    fn test_build_file_entry_directory() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("subdir");
        fs::create_dir(&dir).unwrap();

        let entry = build_file_entry(&dir).unwrap();
        assert!(entry.is_directory);
        assert!(entry.has_children);
    }

    // --- read_directory ---

    #[test]
    fn test_read_directory_basic() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("b.txt"), "").unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        fs::create_dir(tmp.path().join("c_dir")).unwrap();

        let entries = read_directory(tmp.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(entries.len(), 3);
        // Should be sorted alphabetically
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[1].name, "b.txt");
        assert_eq!(entries[2].name, "c_dir");
    }

    #[test]
    fn test_read_directory_excludes_gitignore() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join(".gitignore"), "").unwrap();
        fs::write(tmp.path().join("file.txt"), "").unwrap();

        let entries = read_directory(tmp.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file.txt");
    }

    #[test]
    fn test_read_directory_decodes_encoded_names() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join(".dat_VGVzdA=="), "").unwrap();

        let entries = read_directory(tmp.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Test");
        assert_eq!(entries[0].physical_name, ".dat_VGVzdA==");
        assert!(entries[0].is_encoded);
    }

    #[test]
    fn test_read_directory_not_a_dir() {
        let tmp = create_test_dir();
        let file = tmp.path().join("file.txt");
        fs::write(&file, "").unwrap();

        let result = read_directory(file.to_string_lossy().to_string());
        assert!(result.is_err());
    }

    // --- encode_node ---

    #[test]
    fn test_encode_node_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join("hello.txt");
        fs::write(&file, "content").unwrap();

        let entry = encode_node(file.to_string_lossy().to_string()).unwrap();
        assert!(entry.is_encoded);
        assert_eq!(entry.name, "hello.txt");
        assert!(entry.physical_name.starts_with(".dat_"));
        // Original should no longer exist
        assert!(!file.exists());
        // Encoded file should exist
        assert!(Path::new(&entry.path).exists());
    }

    #[test]
    fn test_encode_node_already_encoded() {
        let tmp = create_test_dir();
        let file = tmp.path().join(".dat_VGVzdA==");
        fs::write(&file, "content").unwrap();

        let entry = encode_node(file.to_string_lossy().to_string()).unwrap();
        // Should remain unchanged
        assert_eq!(entry.physical_name, ".dat_VGVzdA==");
        assert!(file.exists());
    }

    #[test]
    fn test_encode_node_directory_recursive() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("parent");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("child.txt"), "").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();
        fs::write(dir.join("subdir").join("nested.txt"), "").unwrap();

        let entry = encode_node(dir.to_string_lossy().to_string()).unwrap();
        assert!(entry.is_encoded);

        // Children should also be encoded
        let encoded_dir = Path::new(&entry.path);
        let children: Vec<_> = fs::read_dir(encoded_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(children.iter().all(|c| c.starts_with(".dat_")));
    }

    // --- decode_node ---

    #[test]
    fn test_decode_node_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join(".dat_VGVzdA==");
        fs::write(&file, "content").unwrap();

        let entry = decode_node(file.to_string_lossy().to_string()).unwrap();
        assert!(!entry.is_encoded);
        assert_eq!(entry.name, "Test");
        assert_eq!(entry.physical_name, "Test");
        // Original encoded file should no longer exist
        assert!(!file.exists());
        assert!(Path::new(&entry.path).exists());
    }

    #[test]
    fn test_decode_node_already_decoded() {
        let tmp = create_test_dir();
        let file = tmp.path().join("plain.txt");
        fs::write(&file, "content").unwrap();

        let entry = decode_node(file.to_string_lossy().to_string()).unwrap();
        assert_eq!(entry.physical_name, "plain.txt");
        assert!(file.exists());
    }

    #[test]
    fn test_decode_node_directory_recursive() {
        let tmp = create_test_dir();
        let dir = tmp.path().join(".dat_cGFyZW50");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(".dat_Y2hpbGQudHh0"), "").unwrap();

        let entry = decode_node(dir.to_string_lossy().to_string()).unwrap();
        assert!(!entry.is_encoded);
        assert_eq!(entry.name, "parent");

        let decoded_dir = Path::new(&entry.path);
        let children: Vec<_> = fs::read_dir(decoded_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(children, vec!["child.txt"]);
    }

    // --- encode then decode roundtrip ---

    #[test]
    fn test_encode_then_decode_roundtrip() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("my_folder");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "hello").unwrap();

        // Encode
        let encoded = encode_node(dir.to_string_lossy().to_string()).unwrap();
        assert!(encoded.is_encoded);

        // Decode
        let decoded = decode_node(encoded.path.clone()).unwrap();
        assert!(!decoded.is_encoded);
        assert_eq!(decoded.name, "my_folder");

        // Verify file content survived
        let restored_file = Path::new(&decoded.path).join("file.txt");
        assert_eq!(fs::read_to_string(restored_file).unwrap(), "hello");
    }

    // --- rename_node ---

    #[test]
    fn test_rename_node_plain() {
        let tmp = create_test_dir();
        let file = tmp.path().join("old.txt");
        fs::write(&file, "content").unwrap();

        let entry = rename_node(
            file.to_string_lossy().to_string(),
            "new.txt".to_string(),
            false,
        )
        .unwrap();
        assert_eq!(entry.name, "new.txt");
        assert!(!entry.is_encoded);
        assert!(!file.exists());
        assert!(tmp.path().join("new.txt").exists());
    }

    #[test]
    fn test_rename_node_with_encode() {
        let tmp = create_test_dir();
        let file = tmp.path().join("old.txt");
        fs::write(&file, "content").unwrap();

        let entry = rename_node(
            file.to_string_lossy().to_string(),
            "new.txt".to_string(),
            true,
        )
        .unwrap();
        assert_eq!(entry.name, "new.txt");
        assert!(entry.is_encoded);
        assert!(entry.physical_name.starts_with(".dat_"));
    }

    // --- delete_node ---

    #[test]
    fn test_delete_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join("delete_me.txt");
        fs::write(&file, "").unwrap();

        delete_node(file.to_string_lossy().to_string()).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_delete_directory() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("delete_dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("child.txt"), "").unwrap();

        delete_node(dir.to_string_lossy().to_string()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn test_delete_nonexistent() {
        let result = delete_node("/nonexistent/path/file.txt".to_string());
        assert!(result.is_err());
    }

    // --- count_children ---

    #[test]
    fn test_count_children_empty() {
        let tmp = create_test_dir();
        let count = count_children(tmp.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_children_with_items() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        fs::write(tmp.path().join("b.txt"), "").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();

        let count = count_children(tmp.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_children_file_returns_zero() {
        let tmp = create_test_dir();
        let file = tmp.path().join("file.txt");
        fs::write(&file, "").unwrap();

        let count = count_children(file.to_string_lossy().to_string()).unwrap();
        assert_eq!(count, 0);
    }

    // --- move_node ---

    #[test]
    fn test_move_file() {
        let tmp = create_test_dir();
        let file = tmp.path().join("source.txt");
        fs::write(&file, "content").unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();

        let entry = move_node(
            file.to_string_lossy().to_string(),
            target.to_string_lossy().to_string(),
        )
        .unwrap();

        assert!(!file.exists());
        assert_eq!(entry.name, "source.txt");
        assert!(target.join("source.txt").exists());
    }

    #[test]
    fn test_move_directory() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("source_dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("child.txt"), "hello").unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();

        move_node(
            dir.to_string_lossy().to_string(),
            target.to_string_lossy().to_string(),
        )
        .unwrap();

        assert!(!dir.exists());
        let moved = target.join("source_dir").join("child.txt");
        assert_eq!(fs::read_to_string(moved).unwrap(), "hello");
    }

    #[test]
    fn test_move_to_existing_target_fails() {
        let tmp = create_test_dir();
        let file = tmp.path().join("source.txt");
        fs::write(&file, "").unwrap();
        let target = tmp.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("source.txt"), "").unwrap();

        let result = move_node(
            file.to_string_lossy().to_string(),
            target.to_string_lossy().to_string(),
        );
        assert!(result.is_err());
    }

    // --- can_encode_node ---

    // --- search_files ---

    #[test]
    fn test_search_files_basic() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("hello.txt"), "").unwrap();
        fs::write(tmp.path().join("world.txt"), "").unwrap();
        let sub = tmp.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("hello_nested.txt"), "").unwrap();

        let results = search_files(tmp.path().to_string_lossy().to_string(), "hello".to_string()).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.name.to_lowercase().contains("hello")));
    }

    #[test]
    fn test_search_files_case_insensitive() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("Hello.TXT"), "").unwrap();
        fs::write(tmp.path().join("other.txt"), "").unwrap();

        let results = search_files(tmp.path().to_string_lossy().to_string(), "hello".to_string()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Hello.TXT");
    }

    #[test]
    fn test_search_files_encoded_names() {
        let tmp = create_test_dir();
        // .dat_VGVzdA== decodes to "Test"
        fs::write(tmp.path().join(".dat_VGVzdA=="), "").unwrap();
        fs::write(tmp.path().join("other.txt"), "").unwrap();

        let results = search_files(tmp.path().to_string_lossy().to_string(), "test".to_string()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Test");
    }

    #[test]
    fn test_search_files_excludes_gitignore() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join(".gitignore"), "").unwrap();

        let results = search_files(tmp.path().to_string_lossy().to_string(), "gitignore".to_string()).unwrap();
        assert_eq!(results.len(), 0);
    }

    // --- can_encode_node ---

    #[test]
    fn test_can_encode_node_short_name() {
        let tmp = create_test_dir();
        let file = tmp.path().join("short.txt");
        fs::write(&file, "").unwrap();

        let result = can_encode_node(file.to_string_lossy().to_string()).unwrap();
        assert!(result);
    }
}
