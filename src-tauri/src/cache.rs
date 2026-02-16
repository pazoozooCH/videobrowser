use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

pub struct CacheState(pub Mutex<Connection>);

pub fn init_db(app_data_dir: &Path) -> Result<CacheState, String> {
    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let db_path = app_data_dir.join("frame_cache.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open cache database: {}", e))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS frame_cache (
            file_path TEXT NOT NULL,
            file_modified TEXT NOT NULL,
            timestamp_secs REAL NOT NULL,
            frame_jpeg BLOB NOT NULL,
            PRIMARY KEY (file_path, file_modified, timestamp_secs)
        );"
    )
    .map_err(|e| format!("Failed to create cache table: {}", e))?;

    Ok(CacheState(Mutex::new(conn)))
}

pub fn get_cached_frame(
    conn: &Connection,
    path: &str,
    modified: &str,
    timestamp_secs: f64,
) -> Option<Vec<u8>> {
    conn.query_row(
        "SELECT frame_jpeg FROM frame_cache WHERE file_path = ?1 AND file_modified = ?2 AND timestamp_secs = ?3",
        rusqlite::params![path, modified, timestamp_secs],
        |row| row.get(0),
    )
    .ok()
}

pub fn store_frame(
    conn: &Connection,
    path: &str,
    modified: &str,
    timestamp_secs: f64,
    jpeg_data: &[u8],
) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO frame_cache (file_path, file_modified, timestamp_secs, frame_jpeg) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![path, modified, timestamp_secs, jpeg_data],
    );
}
