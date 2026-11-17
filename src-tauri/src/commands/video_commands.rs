//! The preview of one video. A cached video needs one `stat` and the database: no ffprobe and
//! no ffmpeg. The UI's connection is locked only around queries, never around a process or a
//! file read.

use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::Connection;
use tauri::{AppHandle, State};

use crate::cache::fingerprint::fingerprint;
use crate::cache::frames::{self, Probe};
use crate::cache::index;
use crate::cache::mode::FrameMode;
use crate::cache::CacheState;
use crate::commands::cache_commands::{blocking, with_synced_connection};
use crate::media;
use crate::models::video_frame::{Preview, VideoFrame, VideoInfo};

fn lock(state: &CacheState) -> MutexGuard<'_, Connection> {
    state.conn.lock().unwrap_or_else(|p| p.into_inner())
}

/// The videos below a folder, by path, from the index after a quick sync.
#[tauri::command]
pub async fn list_video_files(app: AppHandle, path: String) -> Result<Vec<String>, String> {
    with_synced_connection(app, move |conn| {
        let root = PathBuf::from(path);
        index::sync(conn, &root, false, |_| {})?;
        Ok(index::video_files(conn, &root)?.into_iter().map(|e| e.path).collect())
    })
    .await
}

/// The video of a file. The index knows it if its size and mtime are unchanged; otherwise it is
/// fingerprinted, off the lock.
async fn resolve_video(state: &CacheState, path: &Path) -> Result<i64, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
    if !meta.is_file() {
        return Err(format!("Not a file: {}", path.display()));
    }
    if let Some(id) = index::known_video(&lock(state), path, &meta)? {
        return Ok(id);
    }
    let owned = path.to_path_buf();
    let size = meta.len();
    let fp = blocking(move || fingerprint(&owned, size))
        .await?
        .map_err(|e| format!("Failed to read file: {}", e))?;
    index::record_video(&lock(state), path, &meta, &fp)
}

/// The info of a video, probing it only if it never was, or failed before.
async fn video_info(state: &CacheState, path: &Path, video_id: i64) -> Result<VideoInfo, String> {
    if let Probe::Info(info) = frames::load_probe(&lock(state), video_id)? {
        return Ok(info);
    }
    let owned = path.to_path_buf();
    let result = blocking(move || media::probe(&owned)).await?;
    let conn = lock(state);
    frames::store_probe(&conn, video_id, &result)?;
    result?;
    match frames::load_probe(&conn, video_id)? {
        Probe::Info(info) => Ok(info),
        _ => Err("Probe result not stored".to_string()),
    }
}

fn frame(index: u32, timestamp_secs: f64, jpeg: &[u8]) -> VideoFrame {
    VideoFrame { index, timestamp_secs, data_base64: STANDARD.encode(jpeg) }
}

#[tauri::command]
pub async fn get_video_info(state: State<'_, CacheState>, path: String) -> Result<VideoInfo, String> {
    let path = PathBuf::from(path);
    let video_id = resolve_video(&state, &path).await?;
    video_info(&state, &path, video_id).await
}

/// Everything the preview of a video can show at once: its info, the frames the cache has for
/// the mode, and which ones are still to extract.
#[tauri::command]
pub async fn get_preview(state: State<'_, CacheState>, path: String, mode: FrameMode) -> Result<Preview, String> {
    mode.validate()?;
    let path = PathBuf::from(path);
    let video_id = resolve_video(&state, &path).await?;
    let info = video_info(&state, &path, video_id).await?;
    let timestamps = mode.timestamps(info.duration_secs);
    let key = mode.key();

    let cached = frames::cached_frames(&lock(&state), video_id, &key)?;
    let frames: Vec<VideoFrame> = cached
        .iter()
        .filter(|(idx, _, _)| (*idx as usize) < timestamps.len())
        .map(|(idx, ts, jpeg)| frame(*idx, *ts, jpeg))
        .collect();
    let missing = (0..timestamps.len() as u32)
        .filter(|i| !frames.iter().any(|f| f.index == *i))
        .collect();

    Ok(Preview { info, timestamps, frames, missing })
}

/// One frame of a video in a mode, from the cache or extracted and then cached.
#[tauri::command]
pub async fn extract_video_frame(
    state: State<'_, CacheState>,
    path: String,
    mode: FrameMode,
    index: u32,
) -> Result<VideoFrame, String> {
    mode.validate()?;
    let path = PathBuf::from(path);
    let video_id = resolve_video(&state, &path).await?;
    let info = video_info(&state, &path, video_id).await?;
    let timestamps = mode.timestamps(info.duration_secs);
    let timestamp_secs = *timestamps
        .get(index as usize)
        .ok_or_else(|| format!("No frame {} in this mode", index))?;
    let key = mode.key();

    if let Some(jpeg) = frames::cached_frame(&lock(&state), video_id, &key, index)? {
        return Ok(frame(index, timestamp_secs, &jpeg));
    }

    let owned = path.clone();
    let jpeg = blocking(move || media::extract_frame(&owned, timestamp_secs)).await??;
    frames::store_frame(&lock(&state), video_id, &key, index, timestamp_secs, &jpeg, timestamps.len())?;
    Ok(frame(index, timestamp_secs, &jpeg))
}
