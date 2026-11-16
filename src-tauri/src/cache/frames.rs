//! Probe results, frames, and frame sets of videos, and what the cache holds.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::index::{normalize_root, subtree_bounds};
use super::mode::FrameMode;
use super::{db_err, reclaim_space};
use crate::models::video_frame::VideoInfo;

/// What is known of a video's probe: not yet probed, probed, or failed.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    Unknown,
    Info(VideoInfo),
    Failed(String),
}

pub fn load_probe(conn: &Connection, video_id: i64) -> Result<Probe, String> {
    conn.query_row(
        "SELECT probed, duration_secs, size, width, height, display_aspect_ratio, codec, bitrate,
                framerate, probe_error
         FROM videos WHERE id = ?1",
        [video_id],
        |r| {
            let probed: bool = r.get(0)?;
            if let Some(error) = r.get::<_, Option<String>>(9)? {
                return Ok(Probe::Failed(error));
            }
            if !probed {
                return Ok(Probe::Unknown);
            }
            Ok(Probe::Info(VideoInfo {
                duration_secs: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                file_size_bytes: r.get::<_, i64>(2)? as u64,
                width: r.get(3)?,
                height: r.get(4)?,
                display_aspect_ratio: r.get(5)?,
                codec: r.get(6)?,
                bitrate: r.get::<_, Option<i64>>(7)?.map(|b| b as u64),
                framerate: r.get(8)?,
            }))
        },
    )
    .optional()
    .map_err(db_err)?
    .ok_or_else(|| format!("Unknown video {}", video_id))
}

pub fn store_probe(conn: &Connection, video_id: i64, result: &Result<VideoInfo, String>) -> Result<(), String> {
    match result {
        Ok(info) => conn.execute(
            "UPDATE videos SET probed = 1, probe_error = NULL, duration_secs = ?2, width = ?3,
                 height = ?4, display_aspect_ratio = ?5, codec = ?6, bitrate = ?7, framerate = ?8
             WHERE id = ?1",
            params![
                video_id,
                info.duration_secs,
                info.width,
                info.height,
                info.display_aspect_ratio,
                info.codec,
                info.bitrate.map(|b| b as i64),
                info.framerate
            ],
        ),
        Err(error) => conn.execute(
            "UPDATE videos SET probed = 1, probe_error = ?2 WHERE id = ?1",
            params![video_id, error],
        ),
    }
    .map_err(db_err)?;
    Ok(())
}

/// The stored frames of a video in one mode, by index.
pub fn cached_frames(conn: &Connection, video_id: i64, mode_key: &str) -> Result<Vec<(u32, f64, Vec<u8>)>, String> {
    let mut stmt = conn
        .prepare("SELECT idx, timestamp_secs, jpeg FROM frames WHERE video_id = ?1 AND mode_key = ?2 ORDER BY idx")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![video_id, mode_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

pub fn cached_frame(conn: &Connection, video_id: i64, mode_key: &str, idx: u32) -> Result<Option<Vec<u8>>, String> {
    conn.query_row(
        "SELECT jpeg FROM frames WHERE video_id = ?1 AND mode_key = ?2 AND idx = ?3",
        params![video_id, mode_key, idx],
        |r| r.get(0),
    )
    .optional()
    .map_err(db_err)
}

pub fn cached_indices(conn: &Connection, video_id: i64, mode_key: &str) -> Result<Vec<u32>, String> {
    let mut stmt = conn
        .prepare("SELECT idx FROM frames WHERE video_id = ?1 AND mode_key = ?2")
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![video_id, mode_key], |r| r.get(0))
        .map_err(db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// Stores a frame, and marks the set complete once all `frame_count` frames are there.
pub fn store_frame(
    conn: &Connection,
    video_id: i64,
    mode_key: &str,
    idx: u32,
    timestamp_secs: f64,
    jpeg: &[u8],
    frame_count: usize,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO frames (video_id, mode_key, idx, timestamp_secs, bytes, jpeg)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![video_id, mode_key, idx, timestamp_secs, jpeg.len() as i64, jpeg],
    )
    .map_err(db_err)?;
    let stored: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM frames WHERE video_id = ?1 AND mode_key = ?2",
            params![video_id, mode_key],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    if stored as usize >= frame_count {
        mark_set(conn, video_id, mode_key, frame_count, None)?;
    }
    Ok(())
}

/// Records a set as complete (`error` None) or failed.
pub fn mark_set(conn: &Connection, video_id: i64, mode_key: &str, frame_count: usize, error: Option<&str>) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO frame_sets (video_id, mode_key, frame_count, error) VALUES (?1, ?2, ?3, ?4)",
        params![video_id, mode_key, frame_count as i64, error],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Whether a set needs no more work: complete, or failed before.
pub fn set_settled(conn: &Connection, video_id: i64, mode_key: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM frame_sets WHERE video_id = ?1 AND mode_key = ?2",
        params![video_id, mode_key],
        |_| Ok(()),
    )
    .optional()
    .map(|r| r.is_some())
    .map_err(db_err)
}

pub fn clear_set(conn: &Connection, video_id: i64, mode_key: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM frames WHERE video_id = ?1 AND mode_key = ?2",
        params![video_id, mode_key],
    )
    .map_err(db_err)?;
    conn.execute(
        "DELETE FROM frame_sets WHERE video_id = ?1 AND mode_key = ?2",
        params![video_id, mode_key],
    )
    .map_err(db_err)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeCoverage {
    pub mode: FrameMode,
    pub mode_key: String,
    pub complete: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub path: String,
    pub files: i64,
    pub dirs: i64,
    pub videos: i64,
    pub unprobeable: i64,
    pub frames: i64,
    pub frame_bytes: i64,
    pub modes: Vec<ModeCoverage>,
    pub last_quick_sync: Option<i64>,
    pub last_full_sync: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub db_path: String,
    pub db_bytes: u64,
    pub entries: i64,
    pub videos: i64,
    pub orphan_videos: i64,
    pub frames: i64,
    pub frame_bytes: i64,
    pub root: Option<RootInfo>,
}

const ROOT_VIDEOS: &str = "SELECT DISTINCT video_id FROM entries WHERE path >= ?1 AND path < ?2 AND video_id IS NOT NULL";

pub fn info(conn: &Connection, db_path: &Path, root: Option<&Path>, modes: &[FrameMode]) -> Result<CacheInfo, String> {
    let count = |sql: &str| -> Result<i64, String> { conn.query_row(sql, [], |r| r.get(0)).map_err(db_err) };
    let db_bytes = ["", "-wal"]
        .iter()
        .filter_map(|suffix| std::fs::metadata(format!("{}{}", db_path.display(), suffix)).ok())
        .map(|m| m.len())
        .sum();

    let root = match root {
        Some(root) => Some(root_info(conn, root, modes)?),
        None => None,
    };

    Ok(CacheInfo {
        db_path: db_path.display().to_string(),
        db_bytes,
        entries: count("SELECT COUNT(*) FROM entries")?,
        videos: count("SELECT COUNT(*) FROM videos")?,
        orphan_videos: count(
            "SELECT COUNT(*) FROM videos WHERE NOT EXISTS (SELECT 1 FROM entries e WHERE e.video_id = videos.id)",
        )?,
        frames: count("SELECT COUNT(*) FROM frames")?,
        frame_bytes: count("SELECT COALESCE(SUM(bytes), 0) FROM frames")?,
        root,
    })
}

fn root_info(conn: &Connection, root: &Path, modes: &[FrameMode]) -> Result<RootInfo, String> {
    let root_s = normalize_root(root);
    let (lo, hi) = subtree_bounds(&root_s);
    let in_root = |sql: &str| -> Result<i64, String> {
        conn.query_row(sql, params![lo, hi], |r| r.get(0)).map_err(db_err)
    };

    let files = in_root("SELECT COUNT(*) FROM entries WHERE path >= ?1 AND path < ?2 AND is_dir = 0")?;
    let dirs = in_root("SELECT COUNT(*) FROM entries WHERE path >= ?1 AND path < ?2 AND is_dir = 1")?;
    let videos = in_root(&format!("SELECT COUNT(*) FROM ({})", ROOT_VIDEOS))?;
    let unprobeable = in_root(&format!(
        "SELECT COUNT(*) FROM videos WHERE probe_error IS NOT NULL AND id IN ({})",
        ROOT_VIDEOS
    ))?;
    let frames = in_root(&format!("SELECT COUNT(*) FROM frames WHERE video_id IN ({})", ROOT_VIDEOS))?;
    let frame_bytes = in_root(&format!(
        "SELECT COALESCE(SUM(bytes), 0) FROM frames WHERE video_id IN ({})",
        ROOT_VIDEOS
    ))?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT mode_key, SUM(error IS NULL), SUM(error IS NOT NULL) FROM frame_sets
             WHERE video_id IN ({}) GROUP BY mode_key",
            ROOT_VIDEOS
        ))
        .map_err(db_err)?;
    let by_key: HashMap<String, (i64, i64)> = stmt
        .query_map(params![lo, hi], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))
        .map_err(db_err)?
        .collect::<Result<_, _>>()
        .map_err(db_err)?;
    let modes = modes
        .iter()
        .map(|mode| {
            let key = mode.key();
            let (complete, failed) = by_key.get(&key).copied().unwrap_or((0, 0));
            ModeCoverage { mode: *mode, mode_key: key, complete, failed }
        })
        .collect();

    let (last_quick_sync, last_full_sync) = conn
        .query_row(
            "SELECT last_quick_sync, last_full_sync FROM roots WHERE path = ?1",
            [&root_s],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_err)?
        .unwrap_or((None, None));

    Ok(RootInfo {
        path: root_s,
        files,
        dirs,
        videos,
        unprobeable,
        frames,
        frame_bytes,
        modes,
        last_quick_sync,
        last_full_sync,
    })
}

/// Deletes frames: of the videos below `root`, or all, and of the given modes, or all. Probe
/// results and failures are cleared too, so that the next generate tries again.
pub fn clear(conn: &Connection, root: Option<&Path>, modes: Option<&[FrameMode]>) -> Result<usize, String> {
    let (lo, hi) = match root {
        Some(root) => subtree_bounds(&normalize_root(root)),
        None => (String::new(), String::new()),
    };
    let video_filter = if root.is_some() {
        format!("video_id IN ({})", ROOT_VIDEOS)
    } else {
        "(?1 = ?2)".to_string()
    };
    let keys: Vec<String> = modes.unwrap_or(&[]).iter().map(FrameMode::key).collect();
    let keys_json = serde_json::to_string(&keys).unwrap();
    let mode_filter = if modes.is_some() {
        "mode_key IN (SELECT value FROM json_each(?3))"
    } else {
        "(?3 IS NOT NULL)"
    };

    let tx = conn.unchecked_transaction().map_err(db_err)?;
    let deleted = tx
        .execute(
            &format!("DELETE FROM frames WHERE {} AND {}", video_filter, mode_filter),
            params![lo, hi, keys_json],
        )
        .map_err(db_err)?;
    tx.execute(
        &format!("DELETE FROM frame_sets WHERE {} AND {}", video_filter, mode_filter),
        params![lo, hi, keys_json],
    )
    .map_err(db_err)?;
    let id_filter = if root.is_some() {
        format!("id IN ({})", ROOT_VIDEOS)
    } else {
        "(?1 = ?2)".to_string()
    };
    tx.execute(
        &format!("UPDATE videos SET probe_error = NULL, probed = 0 WHERE probe_error IS NOT NULL AND {}", id_filter),
        params![lo, hi],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    reclaim_space(conn);
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::index::{record_video, sync};
    use crate::cache::test_support::temp_db;
    use std::fs;
    use tempfile::TempDir;

    const NINE: FrameMode = FrameMode::Fixed { count: 9 };
    const MINUTE: FrameMode = FrameMode::Interval { minutes: 1.0 };

    fn info_of(seconds: f64) -> VideoInfo {
        VideoInfo {
            duration_secs: seconds,
            file_size_bytes: 0,
            width: Some(640),
            height: Some(480),
            display_aspect_ratio: None,
            codec: Some("h264".into()),
            bitrate: None,
            framerate: Some("25.00".into()),
        }
    }

    fn video(conn: &Connection, dir: &TempDir, name: &str) -> i64 {
        let path = dir.path().join(name);
        fs::write(&path, name.as_bytes()).unwrap();
        let meta = fs::metadata(&path).unwrap();
        record_video(conn, &path, &meta, name).unwrap()
    }

    #[test]
    fn test_probe_roundtrip() {
        let (_db, conn) = temp_db();
        let dir = TempDir::new().unwrap();
        let id = video(&conn, &dir, "a.mp4");
        assert_eq!(load_probe(&conn, id).unwrap(), Probe::Unknown);
        store_probe(&conn, id, &Ok(info_of(90.0))).unwrap();
        match load_probe(&conn, id).unwrap() {
            Probe::Info(info) => {
                assert_eq!(info.duration_secs, 90.0);
                assert_eq!(info.file_size_bytes, 5);
                assert_eq!(info.framerate.as_deref(), Some("25.00"));
            }
            other => panic!("{:?}", other),
        }
        store_probe(&conn, id, &Err("broken".into())).unwrap();
        assert_eq!(load_probe(&conn, id).unwrap(), Probe::Failed("broken".into()));
    }

    #[test]
    fn test_set_completes_with_last_frame() {
        let (_db, conn) = temp_db();
        let dir = TempDir::new().unwrap();
        let id = video(&conn, &dir, "a.mp4");
        let key = NINE.key();
        for idx in 0..8 {
            store_frame(&conn, id, &key, idx, idx as f64, b"jpg", 9).unwrap();
        }
        assert!(!set_settled(&conn, id, &key).unwrap());
        assert_eq!(cached_indices(&conn, id, &key).unwrap().len(), 8);
        store_frame(&conn, id, &key, 8, 8.0, b"jpg", 9).unwrap();
        assert!(set_settled(&conn, id, &key).unwrap());
        assert_eq!(cached_frames(&conn, id, &key).unwrap().len(), 9);
        assert_eq!(cached_frame(&conn, id, &key, 3).unwrap(), Some(b"jpg".to_vec()));
        clear_set(&conn, id, &key).unwrap();
        assert!(!set_settled(&conn, id, &key).unwrap());
    }

    #[test]
    fn test_info_and_clear() {
        let (db, mut conn) = temp_db();
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/a.mp4"), b"aaa").unwrap();
        fs::write(tmp.path().join("b.mkv"), b"bbb").unwrap();
        sync(&mut conn, tmp.path(), false, |_| {}).unwrap();
        let ids: Vec<i64> = video_files_ids(&conn);
        assert_eq!(ids.len(), 2);

        for &id in &ids {
            store_frame(&conn, id, &NINE.key(), 0, 1.0, b"12345", 1).unwrap();
        }
        mark_set(&conn, ids[0], &MINUTE.key(), 0, Some("ffmpeg failed")).unwrap();

        let db_path = db.path().join("frame_cache.db");
        let info = info(&conn, &db_path, Some(tmp.path()), &[NINE, MINUTE]).unwrap();
        assert_eq!(info.videos, 2);
        assert_eq!(info.frames, 2);
        assert_eq!(info.frame_bytes, 10);
        assert!(info.db_bytes > 0);
        let root = info.root.unwrap();
        assert_eq!((root.files, root.dirs, root.videos), (2, 1, 2));
        assert_eq!((root.modes[0].complete, root.modes[0].failed), (2, 0));
        assert_eq!((root.modes[1].complete, root.modes[1].failed), (0, 1));
        assert!(root.last_quick_sync.is_some());

        // Clearing below sub/ leaves b.mkv's frame.
        assert_eq!(clear(&conn, Some(&tmp.path().join("sub")), None).unwrap(), 1);
        let left: i64 = conn.query_row("SELECT COUNT(*) FROM frames", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 1);
        // Clearing another mode deletes nothing; clearing all of NINE deletes the rest.
        assert_eq!(clear(&conn, None, Some(&[MINUTE])).unwrap(), 0);
        assert_eq!(clear(&conn, None, Some(&[NINE])).unwrap(), 1);
        let sets: i64 = conn.query_row("SELECT COUNT(*) FROM frame_sets", [], |r| r.get(0)).unwrap();
        assert_eq!(sets, 0);
    }

    fn video_files_ids(conn: &Connection) -> Vec<i64> {
        let mut stmt = conn.prepare("SELECT id FROM videos ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }
}
