use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::Connection;
use tauri::{Emitter, State};

use crate::cache::index::{self, EntryRow, EXCLUDED_FILES};
use crate::cache::CacheState;
use crate::commands::cache_commands::with_synced_connection;
use crate::encoding::encoded_dir::{can_encode, encode_name, try_decode_name};
use crate::models::file_entry::FileEntry;

/// Renames made by an operation, in the order made: (old path, new path).
pub type Renames = Vec<(PathBuf, PathBuf)>;

#[tauri::command]
pub fn get_cli_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|arg| Path::new(arg).is_dir())
}

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
        let file_size = if is_directory {
            0
        } else {
            fs::metadata(&entry_path).map(|m| m.len()).unwrap_or(0)
        };
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
            file_size,
        });
    }

    file_entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(file_entries)
}

pub fn encode_node_impl(path: String, renames: &mut Renames) -> Result<FileEntry, String> {
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
        renames.push((node_path.to_path_buf(), new_path.clone()));
        new_path
    } else {
        node_path.to_path_buf()
    };

    // Recursively encode children if directory
    if current_path.is_dir() {
        encode_children_recursive(&current_path, renames)?;
    }

    build_file_entry(&current_path)
}

fn encode_children_recursive(dir: &Path, renames: &mut Renames) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if try_decode_name(&name).is_none() {
            let encoded_name = encode_name(&name);
            let new_path = dir.join(&encoded_name);
            fs::rename(&entry_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;
            renames.push((entry_path.clone(), new_path.clone()));

            if new_path.is_dir() {
                encode_children_recursive(&new_path, renames)?;
            }
        } else if entry_path.is_dir() {
            encode_children_recursive(&entry_path, renames)?;
        }
    }
    Ok(())
}

pub fn decode_node_impl(path: String, renames: &mut Renames) -> Result<FileEntry, String> {
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
        renames.push((node_path.to_path_buf(), new_path.clone()));
        new_path
    } else {
        node_path.to_path_buf()
    };

    // Recursively decode children if directory
    if current_path.is_dir() {
        decode_children_recursive(&current_path, renames)?;
    }

    build_file_entry(&current_path)
}

fn decode_children_recursive(dir: &Path, renames: &mut Renames) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if let Some(decoded_name) = try_decode_name(&name) {
            let new_path = dir.join(&decoded_name);
            fs::rename(&entry_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;
            renames.push((entry_path.clone(), new_path.clone()));

            if new_path.is_dir() {
                decode_children_recursive(&new_path, renames)?;
            }
        } else if entry_path.is_dir() {
            decode_children_recursive(&entry_path, renames)?;
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

pub fn rename_node_impl(path: String, new_name: String, encode: bool) -> Result<FileEntry, String> {
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

pub fn delete_node_impl(path: String) -> Result<(), String> {
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

pub fn move_node_impl(source: String, target_dir: String) -> Result<FileEntry, String> {
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
pub fn open_in_vlc(path: String, start_time: Option<f64>) -> Result<(), String> {
    let node_path = Path::new(&path);
    if !node_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("vlc");
        if let Some(t) = start_time {
            cmd.arg(format!("--start-time={}", t));
        }
        cmd.arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open VLC: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("C:/Program Files/VideoLAN/VLC/vlc.exe");
        if let Some(t) = start_time {
            cmd.arg(format!("--start-time={}", t));
        }
        cmd.arg(format!("file:///{}", path.replace('\\', "/")))
            .spawn()
            .map_err(|e| format!("Failed to open VLC: {}", e))?;
    }

    Ok(())
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchProgress {
    scanned: usize,
    matches: usize,
    done: bool,
}

// Async so that the sync before the query runs off the main thread: a synchronous command
// would block the main GTK/event loop on Linux and freeze the webview.
#[tauri::command]
pub async fn search_files(
    app: tauri::AppHandle,
    window: tauri::Window,
    path: String,
    pattern: String,
) -> Result<Vec<FileEntry>, String> {
    with_synced_connection(app, move |conn| {
        search_files_impl(conn, &path, &pattern, |scanned, matches, done| {
            let _ = window.emit("search-progress", SearchProgress { scanned, matches, done });
        })
    })
    .await
}

/// Search: a quick sync brings the index of `path` up to date, which costs about one `stat` per
/// directory, and the index answers. A name matches when it contains every whitespace-separated
/// term of `pattern`, in any order, so "my file" matches "my test file". `on_progress(scanned,
/// matches, done)` is called during the sync and once with `done = true` at the end.
fn search_files_impl(
    conn: &mut Connection,
    path: &str,
    pattern: &str,
    mut on_progress: impl FnMut(usize, usize, bool),
) -> Result<Vec<FileEntry>, String> {
    let root = Path::new(path);
    let stats = index::sync(conn, root, false, |s| {
        if !s.done {
            on_progress(s.entries, 0, false)
        }
    })?;
    let results: Vec<FileEntry> = index::search(conn, root, pattern)?
        .iter()
        .map(entry_from_row)
        .collect();
    on_progress(stats.entries, results.len(), true);
    Ok(results)
}

pub fn entry_from_row(row: &EntryRow) -> FileEntry {
    let decoded_name = try_decode_name(&row.physical_name);
    FileEntry {
        path: row.path.clone(),
        name: decoded_name.clone().unwrap_or_else(|| row.physical_name.clone()),
        physical_name: row.physical_name.clone(),
        is_directory: row.is_dir,
        is_encoded: decoded_name.is_some(),
        decoded_name,
        has_children: row.is_dir,
        file_size: row.size as u64,
    }
}

// The commands that change the file system, and keep the cache index in step. Should the index
// update fail, the changed directories' mtimes make the next sync catch up.

fn follow_in_index(state: &CacheState, update: impl FnOnce(&Connection) -> Result<(), String>) {
    let conn = state.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Err(e) = update(&conn) {
        eprintln!("Cache index not updated, the next sync will catch up: {}", e);
    }
}

fn follow_renames(state: &CacheState, renames: &Renames) {
    follow_in_index(state, |conn| {
        renames
            .iter()
            .try_for_each(|(old, new)| index::rename_path(conn, old, new))
    });
}

#[tauri::command]
pub fn encode_node(state: State<'_, CacheState>, path: String) -> Result<FileEntry, String> {
    let mut renames = Vec::new();
    let result = encode_node_impl(path, &mut renames);
    follow_renames(&state, &renames);
    result
}

#[tauri::command]
pub fn decode_node(state: State<'_, CacheState>, path: String) -> Result<FileEntry, String> {
    let mut renames = Vec::new();
    let result = decode_node_impl(path, &mut renames);
    follow_renames(&state, &renames);
    result
}

#[tauri::command]
pub fn rename_node(
    state: State<'_, CacheState>,
    path: String,
    new_name: String,
    encode: bool,
) -> Result<FileEntry, String> {
    let entry = rename_node_impl(path.clone(), new_name, encode)?;
    follow_renames(&state, &vec![(PathBuf::from(path), PathBuf::from(&entry.path))]);
    Ok(entry)
}

#[tauri::command]
pub fn move_node(state: State<'_, CacheState>, source: String, target_dir: String) -> Result<FileEntry, String> {
    let entry = move_node_impl(source.clone(), target_dir)?;
    follow_renames(&state, &vec![(PathBuf::from(source), PathBuf::from(&entry.path))]);
    Ok(entry)
}

#[tauri::command]
pub fn delete_node(state: State<'_, CacheState>, path: String) -> Result<(), String> {
    delete_node_impl(path.clone())?;
    follow_in_index(&state, |conn| index::remove_path(conn, Path::new(&path)));
    Ok(())
}

pub fn build_file_entry(path: &Path) -> Result<FileEntry, String> {
    let physical_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid file name")?
        .to_string();
    let is_directory = path.is_dir();
    let file_size = if is_directory {
        0
    } else {
        fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    };
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
        file_size,
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

        let entry = encode_node_impl(file.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
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

        let entry = encode_node_impl(file.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
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

        let entry = encode_node_impl(dir.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
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

        let entry = decode_node_impl(file.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
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

        let entry = decode_node_impl(file.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
        assert_eq!(entry.physical_name, "plain.txt");
        assert!(file.exists());
    }

    #[test]
    fn test_decode_node_directory_recursive() {
        let tmp = create_test_dir();
        let dir = tmp.path().join(".dat_cGFyZW50");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(".dat_Y2hpbGQudHh0"), "").unwrap();

        let entry = decode_node_impl(dir.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
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
        let encoded = encode_node_impl(dir.to_string_lossy().to_string(), &mut Vec::new()).unwrap();
        assert!(encoded.is_encoded);

        // Decode
        let decoded = decode_node_impl(encoded.path.clone(), &mut Vec::new()).unwrap();
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

        let entry = rename_node_impl(
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

        let entry = rename_node_impl(
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

        delete_node_impl(file.to_string_lossy().to_string()).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_delete_directory() {
        let tmp = create_test_dir();
        let dir = tmp.path().join("delete_dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("child.txt"), "").unwrap();

        delete_node_impl(dir.to_string_lossy().to_string()).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn test_delete_nonexistent() {
        let result = delete_node_impl("/nonexistent/path/file.txt".to_string());
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

        let entry = move_node_impl(
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

        move_node_impl(
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

        let result = move_node_impl(
            file.to_string_lossy().to_string(),
            target.to_string_lossy().to_string(),
        );
        assert!(result.is_err());
    }

    // --- can_encode_node ---

    // --- search_files ---

    fn search(root: &str, pattern: &str, progress: impl FnMut(usize, usize, bool)) -> Result<Vec<FileEntry>, String> {
        let (_db, mut conn) = crate::cache::test_support::temp_db();
        search_files_impl(&mut conn, root, pattern, progress)
    }

    #[test]
    fn test_search_files_reports_done_with_totals() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("a.txt"), "").unwrap();
        fs::write(tmp.path().join("b.txt"), "").unwrap();
        let mut last = None;
        search(&tmp.path().to_string_lossy(), "a", |s, m, d| last = Some((s, m, d))).unwrap();
        assert_eq!(last, Some((2, 1, true)));
    }

    #[test]
    fn test_search_files_sees_changes_since_last_search() {
        let tmp = create_test_dir();
        let root = tmp.path().to_string_lossy().to_string();
        let (_db, mut conn) = crate::cache::test_support::temp_db();
        assert!(search_files_impl(&mut conn, &root, "late", |_, _, _| {}).unwrap().is_empty());
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("late.txt"), "").unwrap();
        let found = search_files_impl(&mut conn, &root, "late", |_, _, _| {}).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, sub.join("late.txt").to_string_lossy());
    }

    #[test]
    fn test_search_files_basic() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("hello.txt"), "").unwrap();
        fs::write(tmp.path().join("world.txt"), "").unwrap();
        let sub = tmp.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("hello_nested.txt"), "").unwrap();

        let results = search(&tmp.path().to_string_lossy(), "hello", |_, _, _| {}).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.name.to_lowercase().contains("hello")));
    }

    #[test]
    fn test_search_files_case_insensitive() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("Hello.TXT"), "").unwrap();
        fs::write(tmp.path().join("other.txt"), "").unwrap();

        let results = search(&tmp.path().to_string_lossy(), "hello", |_, _, _| {}).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Hello.TXT");
    }

    #[test]
    fn test_search_files_encoded_names() {
        let tmp = create_test_dir();
        // .dat_VGVzdA== decodes to "Test"
        fs::write(tmp.path().join(".dat_VGVzdA=="), "").unwrap();
        fs::write(tmp.path().join("other.txt"), "").unwrap();

        let results = search(&tmp.path().to_string_lossy(), "test", |_, _, _| {}).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Test");
    }

    #[test]
    fn test_search_files_excludes_gitignore() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join(".gitignore"), "").unwrap();

        let results = search(&tmp.path().to_string_lossy(), "gitignore", |_, _, _| {}).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_files_multiple_terms_order_independent() {
        let tmp = create_test_dir();
        fs::write(tmp.path().join("my test file.txt"), "").unwrap();
        fs::write(tmp.path().join("other document.txt"), "").unwrap();

        let root = tmp.path().to_string_lossy();
        // All of these term sets should match "my test file.txt".
        for query in ["my test", "my file", "my fi", "my le", "test my", "file my test"] {
            let results = search(&root, query, |_, _, _| {}).unwrap();
            assert_eq!(results.len(), 1, "query {:?} should match", query);
            assert_eq!(results[0].name, "my test file.txt", "query {:?}", query);
        }

        // A term not present anywhere excludes the file.
        let none = search(&root, "my missing", |_, _, _| {}).unwrap();
        assert_eq!(none.len(), 0);
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
