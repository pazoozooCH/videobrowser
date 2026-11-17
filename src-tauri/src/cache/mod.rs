//! The cache database: an index of the file trees that were opened, and the preview frames of
//! every video, stored by content so that renames and moves keep them.
//!
//! - `entries`: every file and directory below a synced root, by path. A video file points to
//!   its `videos` row.
//! - `videos`: one row per video content, found by its fingerprint, with the ffprobe result.
//! - `frames` and `frame_sets`: the JPEGs of a video per frame mode, and which sets are complete
//!   or failed.

pub mod fingerprint;
pub mod frames;
pub mod index;
pub mod jobs;
pub mod mode;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::Connection;

const SCHEMA_VERSION: i64 = 2;
const DB_FILE: &str = "frame_cache.db";

pub struct CacheState {
    /// The connection of the UI's own commands. Locked briefly; long jobs open their own.
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
    /// Serialises syncs, so that two never walk the same tree at once.
    pub sync_lock: Mutex<()>,
    pub jobs: jobs::JobControl,
}

impl CacheState {
    /// A new connection to the same database, for work off the UI's connection.
    pub fn connect(&self) -> Result<Connection, String> {
        open(&self.db_path)
    }
}

pub fn init(app_data_dir: &Path) -> Result<CacheState, String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;
    let db_path = app_data_dir.join(DB_FILE);
    let conn = open(&db_path)?;
    Ok(CacheState {
        conn: Mutex::new(conn),
        db_path,
        sync_lock: Mutex::new(()),
        jobs: jobs::JobControl::default(),
    })
}

/// Opens the database, and creates or migrates its schema.
pub fn open(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("Failed to open cache database: {}", e))?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), String> {
    conn.busy_timeout(Duration::from_secs(10)).map_err(db_err)?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")
        .map_err(db_err)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db_err)?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    // Version 0 or 1 kept frames by path and float timestamp. They cannot be mapped to the
    // new keys reliably, so they are dropped, and the file is rewritten with incremental vacuum
    // so that later deletes can give space back.
    conn.execute_batch(
        "DROP TABLE IF EXISTS frame_cache;
         PRAGMA auto_vacuum = INCREMENTAL;
         VACUUM;",
    )
    .map_err(db_err)?;

    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS videos (
             id INTEGER PRIMARY KEY,
             fingerprint TEXT NOT NULL UNIQUE,
             size INTEGER NOT NULL,
             probed INTEGER NOT NULL DEFAULT 0,
             duration_secs REAL,
             width INTEGER,
             height INTEGER,
             display_aspect_ratio TEXT,
             codec TEXT,
             bitrate INTEGER,
             framerate TEXT,
             probe_error TEXT
         );
         CREATE TABLE IF NOT EXISTS entries (
             path TEXT PRIMARY KEY,
             parent_path TEXT NOT NULL,
             physical_name TEXT NOT NULL,
             name_lower TEXT NOT NULL,
             is_dir INTEGER NOT NULL,
             size INTEGER NOT NULL,
             mtime_ns INTEGER NOT NULL,
             video_id INTEGER REFERENCES videos(id) ON DELETE SET NULL
         );
         CREATE INDEX IF NOT EXISTS entries_parent ON entries(parent_path);
         CREATE INDEX IF NOT EXISTS entries_video ON entries(video_id);
         CREATE TABLE IF NOT EXISTS frames (
             video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
             mode_key TEXT NOT NULL,
             idx INTEGER NOT NULL,
             timestamp_secs REAL NOT NULL,
             bytes INTEGER NOT NULL,
             jpeg BLOB NOT NULL,
             PRIMARY KEY (video_id, mode_key, idx)
         );
         CREATE TABLE IF NOT EXISTS frame_sets (
             video_id INTEGER NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
             mode_key TEXT NOT NULL,
             frame_count INTEGER NOT NULL,
             error TEXT,
             PRIMARY KEY (video_id, mode_key)
         );
         CREATE TABLE IF NOT EXISTS roots (
             path TEXT PRIMARY KEY,
             last_quick_sync INTEGER,
             last_full_sync INTEGER
         );
         PRAGMA user_version = 2;
         COMMIT;",
    )
    .map_err(db_err)
}

pub fn db_err(e: rusqlite::Error) -> String {
    format!("Cache database error: {}", e)
}

/// Gives freed pages back to the file system.
pub fn reclaim_space(conn: &Connection) {
    let _ = conn.execute_batch("PRAGMA incremental_vacuum; PRAGMA wal_checkpoint(TRUNCATE);");
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use tempfile::TempDir;

    pub fn temp_db() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = open(&dir.path().join(DB_FILE)).unwrap();
        (dir, conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_migrates_old_frame_cache() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(DB_FILE);
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE frame_cache (file_path TEXT, file_modified TEXT, timestamp_secs REAL,
                 frame_jpeg BLOB, PRIMARY KEY (file_path, file_modified, timestamp_secs));
                 INSERT INTO frame_cache VALUES ('/a.mp4', '1', 1.5, x'FFD8');",
            )
            .unwrap();
        }

        let conn = open(&path).unwrap();
        let old_tables: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'frame_cache'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(old_tables, 0);
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let auto_vacuum: i64 = conn.query_row("PRAGMA auto_vacuum", [], |r| r.get(0)).unwrap();
        assert_eq!(auto_vacuum, 2);
    }

    #[test]
    fn test_reopen_keeps_data() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(DB_FILE);
        {
            let conn = open(&path).unwrap();
            conn.execute("INSERT INTO videos (fingerprint, size) VALUES ('x', 1)", []).unwrap();
        }
        let conn = open(&path).unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM videos", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }
}
