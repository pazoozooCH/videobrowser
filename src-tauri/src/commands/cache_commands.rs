//! The global cache functions: sync, generate, cancel, info, and clear.

use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::cache::frames::{self, CacheInfo};
use crate::cache::index::{self, SyncStats};
use crate::cache::jobs::{self, GenerateProgress, Throttle};
use crate::cache::mode::FrameMode;
use crate::cache::CacheState;
use crate::media;

/// ffmpeg processes a generate job runs at once.
const GENERATE_WORKERS: usize = 2;

/// At most this often a generate job reports progress; the webview handles every event on its
/// main thread.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {}", e))
}

/// Runs `f` off the main thread on a connection of its own, holding the sync lock, so that a
/// sync inside it never runs alongside another.
pub async fn with_synced_connection<R: Runtime, T: Send + 'static>(
    app: AppHandle<R>,
    f: impl FnOnce(&mut Connection) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    blocking(move || {
        let state = app.state::<CacheState>();
        let _sync = state.sync_lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut conn = state.connect()?;
        f(&mut conn)
    })
    .await?
}

/// Brings the index of `root` in line with the file system. Quick by default: only directories
/// whose mtime changed are listed. `full` lists all, compares every file, and drops the frames
/// of videos that are gone. Emits `cache-sync-progress`.
#[tauri::command]
pub async fn sync_cache<R: Runtime>(app: AppHandle<R>, root: String, full: bool) -> Result<SyncStats, String> {
    let emitter = app.clone();
    with_synced_connection(app, move |conn| {
        index::sync(conn, &PathBuf::from(root), full, |stats| {
            let _ = emitter.emit("cache-sync-progress", stats);
        })
    })
    .await
}

/// Generates the frames of every video below `root` for each mode, skipping what is cached;
/// `force` generates them anew. Syncs first, so new files are included. Emits
/// `cache-generate-progress`; `cancel_cache_job` stops it.
#[tauri::command]
pub async fn generate_cache<R: Runtime>(
    app: AppHandle<R>,
    root: String,
    modes: Vec<FrameMode>,
    force: bool,
) -> Result<GenerateProgress, String> {
    for mode in &modes {
        mode.validate()?;
    }
    blocking(move || {
        let state = app.state::<CacheState>();
        let job = state.jobs.start()?;
        let root = PathBuf::from(root);
        {
            let _sync = state.sync_lock.lock().unwrap_or_else(|p| p.into_inner());
            let mut conn = state.connect()?;
            index::sync(&mut conn, &root, false, |stats| {
                let _ = app.emit("cache-sync-progress", stats);
            })?;
        }
        let conn = state.connect()?;
        let mut throttle = Throttle::new(PROGRESS_INTERVAL);
        jobs::generate(
            &conn,
            &root,
            &modes,
            force,
            GENERATE_WORKERS,
            job.cancel_flag(),
            &media::probe,
            &media::extract_frame,
            &mut |progress| {
                if throttle.ready(progress.done) {
                    let _ = app.emit("cache-generate-progress", progress.without_errors());
                }
            },
        )
    })
    .await?
}

#[tauri::command]
pub fn cancel_cache_job(state: State<'_, CacheState>) {
    state.jobs.cancel();
}

#[tauri::command]
pub fn cache_job_running(state: State<'_, CacheState>) -> bool {
    state.jobs.is_running()
}

/// Size and contents of the cache, and with `root`, what is cached below it per mode.
#[tauri::command]
pub async fn cache_info<R: Runtime>(app: AppHandle<R>, root: Option<String>, modes: Vec<FrameMode>) -> Result<CacheInfo, String> {
    blocking(move || {
        let state = app.state::<CacheState>();
        let conn = state.connect()?;
        frames::info(&conn, &state.db_path, root.as_ref().map(PathBuf::from).as_deref(), &modes)
    })
    .await?
}

/// Deletes frames below `root` (or all), of `modes` (or all). Returns how many.
#[tauri::command]
pub async fn clear_cache<R: Runtime>(app: AppHandle<R>, root: Option<String>, modes: Option<Vec<FrameMode>>) -> Result<usize, String> {
    blocking(move || {
        let state = app.state::<CacheState>();
        let _job = state.jobs.start()?;
        let conn = state.connect()?;
        frames::clear(&conn, root.as_ref().map(PathBuf::from).as_deref(), modes.as_deref())
    })
    .await?
}
