//! The index of files and directories, and how it follows the file system.
//!
//! A quick sync trusts a directory whose mtime is unchanged: adding, removing, or renaming an
//! entry changes the mtime of its parent, so only the directories that changed are listed, and
//! the cost is one `stat` per directory. A full sync lists every directory and compares every
//! file, which also catches a file rewritten in place, and drops videos no entry points to.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use super::{db_err, fingerprint::fingerprint, reclaim_space};
use crate::encoding::encoded_dir::try_decode_name;
use crate::media::is_video_name;

pub const EXCLUDED_FILES: &[&str] = &[".gitignore"];

const PROGRESS_EVERY_DIRS: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct EntryRow {
    pub path: String,
    pub parent_path: String,
    pub physical_name: String,
    pub is_dir: bool,
    pub size: i64,
    pub mtime_ns: i64,
    pub video_id: Option<i64>,
}

impl EntryRow {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(EntryRow {
            path: row.get(0)?,
            parent_path: row.get(1)?,
            physical_name: row.get(2)?,
            is_dir: row.get(3)?,
            size: row.get(4)?,
            mtime_ns: row.get(5)?,
            video_id: row.get(6)?,
        })
    }

    fn new(path: &Path, meta: &fs::Metadata) -> Self {
        let is_dir = meta.is_dir();
        EntryRow {
            path: path_str(path),
            parent_path: path.parent().map(path_str).unwrap_or_default(),
            physical_name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_str(path)),
            is_dir,
            size: if is_dir { 0 } else { meta.len() as i64 },
            mtime_ns: mtime_ns(meta),
            video_id: None,
        }
    }

    pub fn display_name(&self) -> String {
        display_name(&self.physical_name)
    }
}

const ENTRY_COLUMNS: &str = "path, parent_path, physical_name, is_dir, size, mtime_ns, video_id";

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStats {
    pub full: bool,
    pub dirs: usize,
    pub listed_dirs: usize,
    pub entries: usize,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub reused_videos: usize,
    pub pruned_videos: usize,
    pub done: bool,
}

pub fn display_name(physical_name: &str) -> String {
    try_decode_name(physical_name).unwrap_or_else(|| physical_name.to_string())
}

pub fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The root as stored: no trailing separator, except for a file-system root.
pub fn normalize_root(root: &Path) -> String {
    let s = path_str(root);
    let trimmed = s.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        s
    } else {
        trimmed.to_string()
    }
}

/// `[lo, hi)` such that every path strictly below `root` sorts in it, byte-wise.
pub fn subtree_bounds(root: &str) -> (String, String) {
    let base = root.trim_end_matches(MAIN_SEPARATOR);
    let next = char::from_u32(MAIN_SEPARATOR as u32 + 1).unwrap();
    (format!("{}{}", base, MAIN_SEPARATOR), format!("{}{}", base, next))
}

pub fn mtime_ns(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn upsert_entry(conn: &Connection, row: &EntryRow) -> Result<(), String> {
    conn.execute(
        "INSERT INTO entries (path, parent_path, physical_name, name_lower, is_dir, size, mtime_ns, video_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(path) DO UPDATE SET
             parent_path = excluded.parent_path, physical_name = excluded.physical_name,
             name_lower = excluded.name_lower, is_dir = excluded.is_dir, size = excluded.size,
             mtime_ns = excluded.mtime_ns, video_id = excluded.video_id",
        params![
            row.path,
            row.parent_path,
            row.physical_name,
            row.display_name().to_lowercase(),
            row.is_dir,
            row.size,
            row.mtime_ns,
            row.video_id
        ],
    )
    .map_err(db_err)?;
    Ok(())
}

fn remove_subtree(conn: &Connection, path: &str) -> Result<usize, String> {
    let (lo, hi) = subtree_bounds(path);
    conn.execute(
        "DELETE FROM entries WHERE path = ?1 OR (path >= ?2 AND path < ?3)",
        params![path, lo, hi],
    )
    .map_err(db_err)
}

/// The `videos` row of a fingerprint, created if new. Returns the id and whether it existed.
pub fn video_for_fingerprint(conn: &Connection, fingerprint: &str, size: i64) -> Result<(i64, bool), String> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM videos WHERE fingerprint = ?1", [fingerprint], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    if let Some(id) = existing {
        return Ok((id, true));
    }
    conn.execute(
        "INSERT INTO videos (fingerprint, size) VALUES (?1, ?2)",
        params![fingerprint, size],
    )
    .map_err(db_err)?;
    Ok((conn.last_insert_rowid(), false))
}

pub fn get_entry(conn: &Connection, path: &str) -> Result<Option<EntryRow>, String> {
    conn.query_row(
        &format!("SELECT {} FROM entries WHERE path = ?1", ENTRY_COLUMNS),
        [path],
        EntryRow::from_row,
    )
    .optional()
    .map_err(db_err)
}

fn load_subtree(conn: &Connection, root: &str) -> Result<HashMap<String, EntryRow>, String> {
    let (lo, hi) = subtree_bounds(root);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM entries WHERE path = ?1 OR (path >= ?2 AND path < ?3)",
            ENTRY_COLUMNS
        ))
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![root, lo, hi], EntryRow::from_row)
        .map_err(db_err)?;
    let mut map = HashMap::new();
    for row in rows {
        let row = row.map_err(db_err)?;
        map.insert(row.path.clone(), row);
    }
    Ok(map)
}

/// Brings the index of `root` in line with the file system. See the module documentation.
pub fn sync(
    conn: &mut Connection,
    root: &Path,
    full: bool,
    mut progress: impl FnMut(&SyncStats),
) -> Result<SyncStats, String> {
    let root_s = normalize_root(root);
    let root_path = PathBuf::from(&root_s);
    if !fs::metadata(&root_path).map(|m| m.is_dir()).unwrap_or(false) {
        return Err(format!("Not a directory: {}", root_s));
    }

    let known = load_subtree(conn, &root_s)?;
    let mut children: HashMap<&str, Vec<&EntryRow>> = HashMap::new();
    for row in known.values() {
        children.entry(row.parent_path.as_str()).or_default().push(row);
    }

    let mut stats = SyncStats { full, ..Default::default() };
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending: Vec<EntryRow> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root_path.clone()];
    seen.insert(root_s.clone());

    while let Some(dir) = stack.pop() {
        let meta = match fs::metadata(&dir) {
            Ok(m) if m.is_dir() => m,
            _ => continue,
        };
        stats.dirs += 1;
        if stats.dirs.is_multiple_of(PROGRESS_EVERY_DIRS) {
            progress(&stats);
        }

        let dir_s = path_str(&dir);
        let dir_row = EntryRow::new(&dir, &meta);
        let known_dir = known.get(&dir_s);
        let dir_unchanged = known_dir.is_some_and(|k| k.is_dir && k.mtime_ns == dir_row.mtime_ns);

        let listing = if dir_unchanged && !full { None } else { fs::read_dir(&dir).ok() };
        let Some(listing) = listing else {
            // Unchanged, or not readable: keep what is known below it.
            for child in children.get(dir_s.as_str()).into_iter().flatten() {
                seen.insert(child.path.clone());
                stats.entries += 1;
                if child.is_dir {
                    stack.push(PathBuf::from(&child.path));
                }
            }
            continue;
        };

        stats.listed_dirs += 1;
        if !dir_unchanged {
            pending.push(dir_row);
        }

        for item in listing.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if EXCLUDED_FILES.contains(&name.as_str()) {
                continue;
            }
            let path = item.path();
            let Ok(meta) = fs::metadata(&path) else { continue };
            let row = EntryRow::new(&path, &meta);
            seen.insert(row.path.clone());
            stats.entries += 1;

            let is_video = !row.is_dir && is_video_name(&row.physical_name);
            match known.get(&row.path) {
                Some(k) if k.is_dir && row.is_dir => {}
                Some(k)
                    if !k.is_dir
                        && !row.is_dir
                        && k.size == row.size
                        && k.mtime_ns == row.mtime_ns
                        && (k.video_id.is_some() || !is_video || !full) => {}
                Some(_) => {
                    stats.changed += 1;
                    if !row.is_dir {
                        pending.push(row.clone());
                    }
                }
                None => {
                    stats.added += 1;
                    if !row.is_dir {
                        pending.push(row.clone());
                    }
                }
            }
            if row.is_dir {
                stack.push(path);
            }
        }
    }

    let removed: Vec<&String> = known.keys().filter(|p| !seen.contains(*p)).collect();
    stats.removed = removed.len();

    // Fingerprints read the files, so they are taken before the write transaction starts.
    let fingerprints: Vec<Option<String>> = pending
        .iter()
        .map(|row| {
            if row.is_dir || !is_video_name(&row.physical_name) {
                None
            } else {
                fingerprint(Path::new(&row.path), row.size as u64).ok()
            }
        })
        .collect();

    let tx = conn.transaction().map_err(db_err)?;
    for path in &removed {
        tx.execute("DELETE FROM entries WHERE path = ?1", [path]).map_err(db_err)?;
    }
    for (mut row, fp) in pending.into_iter().zip(fingerprints) {
        if let Some(fp) = fp {
            let (id, existed) = video_for_fingerprint(&tx, &fp, row.size)?;
            if existed {
                stats.reused_videos += 1;
            }
            row.video_id = Some(id);
        }
        upsert_entry(&tx, &row)?;
    }
    let column = if full { "last_full_sync" } else { "last_quick_sync" };
    tx.execute(
        &format!(
            "INSERT INTO roots (path, {0}) VALUES (?1, ?2) ON CONFLICT(path) DO UPDATE SET {0} = excluded.{0}",
            column
        ),
        params![root_s, now_secs()],
    )
    .map_err(db_err)?;
    if full {
        stats.pruned_videos = tx
            .execute(
                "DELETE FROM videos WHERE NOT EXISTS (SELECT 1 FROM entries e WHERE e.video_id = videos.id)",
                [],
            )
            .map_err(db_err)?;
    }
    tx.commit().map_err(db_err)?;

    if stats.pruned_videos > 0 {
        reclaim_space(conn);
    }
    stats.done = true;
    progress(&stats);
    Ok(stats)
}

/// Follows a rename or move the app made: the entry and everything below it change path, and
/// keep their videos.
pub fn rename_path(conn: &Connection, old: &Path, new: &Path) -> Result<(), String> {
    let old_s = path_str(old);
    let new_s = path_str(new);
    if old_s == new_s {
        return Ok(());
    }
    let new_name = new
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| new_s.clone());
    let new_parent = new.parent().map(path_str).unwrap_or_default();

    let tx = conn.unchecked_transaction().map_err(db_err)?;
    remove_subtree(&tx, &new_s)?;
    let (lo, hi) = subtree_bounds(&old_s);
    // substr counts characters, not bytes.
    let tail_from = old_s.chars().count() as i64 + 1;
    tx.execute(
        "UPDATE entries SET path = ?1 || substr(path, ?2), parent_path = ?1 || substr(parent_path, ?2)
         WHERE path >= ?3 AND path < ?4",
        params![new_s, tail_from, lo, hi],
    )
    .map_err(db_err)?;
    tx.execute(
        "UPDATE entries SET path = ?1, parent_path = ?2, physical_name = ?3, name_lower = ?4 WHERE path = ?5",
        params![new_s, new_parent, new_name, display_name(&new_name).to_lowercase(), old_s],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)
}

/// Follows a delete the app made. The videos stay until a full sync, so that putting a file
/// back finds its frames.
pub fn remove_path(conn: &Connection, path: &Path) -> Result<(), String> {
    remove_subtree(conn, &path_str(path)).map(|_| ())
}

/// The video of a file, if the index knows it and its size and mtime are unchanged.
pub fn known_video(conn: &Connection, path: &Path, meta: &fs::Metadata) -> Result<Option<i64>, String> {
    let current = EntryRow::new(path, meta);
    Ok(get_entry(conn, &current.path)?.and_then(|k| {
        (!k.is_dir && k.size == current.size && k.mtime_ns == current.mtime_ns)
            .then_some(k.video_id)
            .flatten()
    }))
}

/// Records a file the index did not know, or that changed, with its fingerprint.
pub fn record_video(conn: &Connection, path: &Path, meta: &fs::Metadata, fingerprint: &str) -> Result<i64, String> {
    let mut row = EntryRow::new(path, meta);
    let (id, _) = video_for_fingerprint(conn, fingerprint, row.size)?;
    row.video_id = Some(id);
    upsert_entry(conn, &row)?;
    Ok(id)
}

/// Entries below `root` whose display name contains every term, case-insensitively.
pub fn search(conn: &Connection, root: &Path, pattern: &str) -> Result<Vec<EntryRow>, String> {
    let terms: Vec<String> = pattern.to_lowercase().split_whitespace().map(String::from).collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let (lo, hi) = subtree_bounds(&normalize_root(root));
    let mut sql = format!("SELECT {} FROM entries WHERE path >= ?1 AND path < ?2", ENTRY_COLUMNS);
    for i in 0..terms.len() {
        sql.push_str(&format!(" AND instr(name_lower, ?{}) > 0", i + 3));
    }
    let mut values: Vec<&dyn rusqlite::ToSql> = vec![&lo, &hi];
    values.extend(terms.iter().map(|t| t as &dyn rusqlite::ToSql));

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(values.as_slice(), EntryRow::from_row)
        .map_err(db_err)?;
    let mut results = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
    results.sort_by_cached_key(|r| r.display_name().to_lowercase());
    Ok(results)
}

/// The video files below `root`, by path.
pub fn video_files(conn: &Connection, root: &Path) -> Result<Vec<EntryRow>, String> {
    let (lo, hi) = subtree_bounds(&normalize_root(root));
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} FROM entries WHERE path >= ?1 AND path < ?2 AND is_dir = 0 ORDER BY path",
            ENTRY_COLUMNS
        ))
        .map_err(db_err)?;
    let rows = stmt.query_map(params![lo, hi], EntryRow::from_row).map_err(db_err)?;
    let mut results = Vec::new();
    for row in rows {
        let row = row.map_err(db_err)?;
        if is_video_name(&row.physical_name) {
            results.push(row);
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::test_support::temp_db;
    use crate::encoding::encoded_dir::encode_name;
    use std::time::Duration;
    use tempfile::TempDir;

    fn quick(conn: &mut Connection, root: &Path) -> SyncStats {
        sync(conn, root, false, |_| {}).unwrap()
    }

    fn video_of(conn: &Connection, path: &Path) -> Option<i64> {
        get_entry(conn, &path_str(path)).unwrap().and_then(|e| e.video_id)
    }

    /// Directory mtimes can be coarse; make sure a change is visible as one.
    fn touch_later(path: &Path) {
        let f = fs::File::open(path).unwrap();
        let later = SystemTime::now() + Duration::from_secs(5);
        f.set_modified(later).unwrap();
    }

    fn tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        fs::write(tmp.path().join("a/movie.mp4"), b"movie-one").unwrap();
        fs::write(tmp.path().join("a/b/notes.txt"), b"text").unwrap();
        fs::write(tmp.path().join(encode_name("Secret Film.mkv")), b"movie-two").unwrap();
        fs::write(tmp.path().join(".gitignore"), b"").unwrap();
        tmp
    }

    #[test]
    fn test_first_sync_indexes_everything() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        let stats = quick(&mut conn, tmp.path());
        assert_eq!(stats.added, 5); // a, a/b, a/movie.mp4, a/b/notes.txt, the encoded file
        assert_eq!(stats.listed_dirs, 3);
        assert!(video_of(&conn, &tmp.path().join("a/movie.mp4")).is_some());
        assert!(video_of(&conn, &tmp.path().join(encode_name("Secret Film.mkv"))).is_some());
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/b/notes.txt"))).unwrap().unwrap().video_id.is_none());
        assert!(get_entry(&conn, &path_str(&tmp.path().join(".gitignore"))).unwrap().is_none());
    }

    #[test]
    fn test_quick_sync_skips_unchanged_directories() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        let again = quick(&mut conn, tmp.path());
        assert_eq!(again.listed_dirs, 0);
        assert_eq!(again.dirs, 3);
        assert_eq!((again.added, again.changed, again.removed), (0, 0, 0));
    }

    #[test]
    fn test_quick_sync_sees_add_and_remove() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());

        fs::remove_file(tmp.path().join("a/b/notes.txt")).unwrap();
        fs::write(tmp.path().join("a/b/new.mov"), b"movie-three").unwrap();
        touch_later(&tmp.path().join("a/b"));

        let stats = quick(&mut conn, tmp.path());
        assert_eq!(stats.listed_dirs, 1);
        assert_eq!((stats.added, stats.removed), (1, 1));
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/b/notes.txt"))).unwrap().is_none());
        assert!(video_of(&conn, &tmp.path().join("a/b/new.mov")).is_some());
    }

    #[test]
    fn test_outside_move_keeps_video() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        let before = video_of(&conn, &tmp.path().join("a/movie.mp4")).unwrap();

        fs::rename(tmp.path().join("a"), tmp.path().join("a/../moved")).unwrap();
        touch_later(tmp.path());

        let stats = quick(&mut conn, tmp.path());
        assert_eq!(stats.reused_videos, 1);
        assert_eq!(video_of(&conn, &tmp.path().join("moved/movie.mp4")), Some(before));
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/movie.mp4"))).unwrap().is_none());
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/b/notes.txt"))).unwrap().is_none());
    }

    #[test]
    fn test_full_sync_sees_rewrite_and_prunes() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        let movie = tmp.path().join("a/movie.mp4");
        let before = video_of(&conn, &movie).unwrap();

        // Rewritten in place: the parent's mtime does not change, so only a full sync sees it.
        fs::write(&movie, b"another movie").unwrap();
        touch_later(&movie);
        let stats = sync(&mut conn, tmp.path(), true, |_| {}).unwrap();
        assert_eq!(stats.changed, 1);
        assert_eq!(stats.pruned_videos, 1);
        let after = video_of(&conn, &movie).unwrap();
        assert_ne!(before, after);
        let old: i64 = conn
            .query_row("SELECT COUNT(*) FROM videos WHERE id = ?1", [before], |r| r.get(0))
            .unwrap();
        assert_eq!(old, 0);
    }

    #[test]
    fn test_rename_path_moves_subtree() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        let video = video_of(&conn, &tmp.path().join("a/movie.mp4"));

        let new_dir = tmp.path().join(encode_name("ä"));
        fs::rename(tmp.path().join("a"), &new_dir).unwrap();
        rename_path(&conn, &tmp.path().join("a"), &new_dir).unwrap();

        assert_eq!(video_of(&conn, &new_dir.join("movie.mp4")), video);
        let notes = get_entry(&conn, &path_str(&new_dir.join("b/notes.txt"))).unwrap().unwrap();
        assert_eq!(notes.parent_path, path_str(&new_dir.join("b")));
        let top = get_entry(&conn, &path_str(&new_dir)).unwrap().unwrap();
        assert_eq!(top.display_name(), "ä");
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/movie.mp4"))).unwrap().is_none());

        // The index agrees with the disk: a quick sync changes nothing but the root listing.
        let stats = quick(&mut conn, tmp.path());
        assert_eq!((stats.added, stats.changed, stats.removed), (0, 0, 0));
    }

    #[test]
    fn test_rename_path_does_not_touch_siblings_with_same_prefix() {
        let (_db, mut conn) = temp_db();
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("a")).unwrap();
        fs::create_dir_all(tmp.path().join("a_b")).unwrap();
        fs::write(tmp.path().join("a_b/x.mp4"), b"x").unwrap();
        quick(&mut conn, tmp.path());
        rename_path(&conn, &tmp.path().join("a"), &tmp.path().join("c")).unwrap();
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a_b/x.mp4"))).unwrap().is_some());
    }

    #[test]
    fn test_remove_path() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        remove_path(&conn, &tmp.path().join("a")).unwrap();
        assert!(get_entry(&conn, &path_str(&tmp.path().join("a/b/notes.txt"))).unwrap().is_none());
        let videos: i64 = conn.query_row("SELECT COUNT(*) FROM videos", [], |r| r.get(0)).unwrap();
        assert_eq!(videos, 2, "videos stay until a full sync");
    }

    #[test]
    fn test_known_and_record_video() {
        let (_db, conn) = temp_db();
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("x.mp4");
        fs::write(&file, b"data").unwrap();
        let meta = fs::metadata(&file).unwrap();
        assert_eq!(known_video(&conn, &file, &meta).unwrap(), None);
        let id = record_video(&conn, &file, &meta, "fp").unwrap();
        assert_eq!(known_video(&conn, &file, &meta).unwrap(), Some(id));

        fs::write(&file, b"other").unwrap();
        touch_later(&file);
        let meta = fs::metadata(&file).unwrap();
        assert_eq!(known_video(&conn, &file, &meta).unwrap(), None);
    }

    #[test]
    fn test_search_terms_and_decoded_names() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        fs::write(tmp.path().join("my test file.txt"), b"").unwrap();
        fs::write(tmp.path().join("HELLO.txt"), b"").unwrap();
        quick(&mut conn, tmp.path());

        let names = |q: &str| -> Vec<String> {
            search(&conn, tmp.path(), q).unwrap().iter().map(|r| r.display_name()).collect()
        };
        assert_eq!(names("secret"), vec!["Secret Film.mkv"]);
        assert_eq!(names("hello"), vec!["HELLO.txt"]);
        assert_eq!(names("file my"), vec!["my test file.txt"]);
        assert_eq!(names("FILE   TEST"), vec!["my test file.txt"]);
        assert!(names("my missing").is_empty());
        assert!(names("gitignore").is_empty());
        assert!(names("   ").is_empty());
        // The root itself is not a result, and the search stays below the given folder.
        assert!(search(&conn, &tmp.path().join("a/b"), "movie").unwrap().is_empty());
    }

    #[test]
    fn test_video_files() {
        let (_db, mut conn) = temp_db();
        let tmp = tree();
        quick(&mut conn, tmp.path());
        let videos = video_files(&conn, tmp.path()).unwrap();
        assert_eq!(videos.len(), 2);
        assert!(videos.windows(2).all(|w| w[0].path < w[1].path));
    }

    #[test]
    fn test_subtree_bounds() {
        let sep = MAIN_SEPARATOR;
        let (lo, hi) = subtree_bounds(&format!("{sep}x"));
        let inside = format!("{sep}x{sep}y");
        let sibling = format!("{sep}x_y");
        assert!(inside >= lo && inside < hi);
        assert!(!(sibling >= lo && sibling < hi));
    }
}
