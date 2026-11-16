//! The long cache jobs: generating the frames of every video below a folder.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde::Serialize;

use super::fingerprint::fingerprint;
use super::frames::{self, Probe};
use super::index::{self, EntryRow};
use super::mode::FrameMode;
use crate::models::video_frame::VideoInfo;

const MAX_REPORTED_ERRORS: usize = 50;

/// One generate job at a time, and a way to stop it.
#[derive(Default)]
pub struct JobControl {
    running: AtomicBool,
    cancel: AtomicBool,
}

impl JobControl {
    pub fn start(&self) -> Result<JobGuard<'_>, String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("A cache job is already running".to_string());
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(JobGuard(self))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

pub struct JobGuard<'a>(&'a JobControl);

impl JobGuard<'_> {
    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.0.cancel
    }
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateProgress {
    pub videos_total: usize,
    pub videos_done: usize,
    pub frames_done: usize,
    pub frames_failed: usize,
    pub videos_failed: usize,
    pub current: Option<String>,
    pub errors: Vec<String>,
    pub done: bool,
    pub cancelled: bool,
}

impl GenerateProgress {
    /// The same report without the error messages, for the frequent progress events: the
    /// counts tell how many failed, and the final result carries the messages.
    pub fn without_errors(&self) -> GenerateProgress {
        GenerateProgress { errors: Vec::new(), current: self.current.clone(), ..*self }
    }

    fn error(&mut self, name: &str, message: &str) {
        if self.errors.len() < MAX_REPORTED_ERRORS {
            self.errors.push(format!("{}: {}", name, message));
        }
    }
}

/// Lets a progress report through at most once per interval, and always the last one, so that
/// a job of many quick steps does not flood the webview with events.
pub struct Throttle {
    every: Duration,
    last: Option<Instant>,
}

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Throttle { every, last: None }
    }

    pub fn ready(&mut self, last_report: bool) -> bool {
        let now = Instant::now();
        let due = self.last.is_none_or(|t| now.duration_since(t) >= self.every);
        if due || last_report {
            self.last = Some(now);
            return true;
        }
        false
    }
}

pub type ProbeFn<'a> = &'a (dyn Fn(&Path) -> Result<VideoInfo, String> + Sync);
pub type ExtractFn<'a> = &'a (dyn Fn(&Path, f64) -> Result<Vec<u8>, String> + Sync);

/// The video of a file, fingerprinting it if the index does not know it as it is now.
pub fn video_id_for_file(conn: &Connection, path: &Path) -> Result<i64, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {}", e))?;
    if let Some(id) = index::known_video(conn, path, &meta)? {
        return Ok(id);
    }
    let fp = fingerprint(path, meta.len()).map_err(|e| format!("Failed to read file: {}", e))?;
    index::record_video(conn, path, &meta, &fp)
}

/// Generates the missing frames of every video below `root`, for every mode. With `force`, the
/// existing frames are generated anew and earlier failures are retried.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    conn: &Connection,
    root: &Path,
    modes: &[FrameMode],
    force: bool,
    workers: usize,
    cancel: &AtomicBool,
    probe: ProbeFn,
    extract: ExtractFn,
    progress: &mut dyn FnMut(&GenerateProgress),
) -> Result<GenerateProgress, String> {
    for mode in modes {
        mode.validate()?;
    }
    let files = index::video_files(conn, root)?;
    let mut state = GenerateProgress { videos_total: files.len(), ..Default::default() };
    progress(&state);

    let mut done_ids: HashSet<i64> = HashSet::new();
    for file in &files {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let name = file.display_name();
        state.current = Some(name.clone());
        progress(&state);

        match generate_one(conn, file, modes, force, workers, cancel, probe, extract, &mut done_ids, &mut state, progress) {
            Ok(()) => {}
            Err(message) => {
                state.videos_failed += 1;
                state.error(&name, &message);
            }
        }
        state.videos_done += 1;
        progress(&state);
    }

    state.current = None;
    state.cancelled = cancel.load(Ordering::SeqCst);
    state.done = true;
    progress(&state);
    Ok(state)
}

#[allow(clippy::too_many_arguments)]
fn generate_one(
    conn: &Connection,
    file: &EntryRow,
    modes: &[FrameMode],
    force: bool,
    workers: usize,
    cancel: &AtomicBool,
    probe: ProbeFn,
    extract: ExtractFn,
    done_ids: &mut HashSet<i64>,
    state: &mut GenerateProgress,
    progress: &mut dyn FnMut(&GenerateProgress),
) -> Result<(), String> {
    let path = Path::new(&file.path);
    let video_id = match file.video_id {
        Some(id) if path.is_file() => id,
        _ => video_id_for_file(conn, path)?,
    };
    // Copies of the same content share their frames.
    if !done_ids.insert(video_id) {
        return Ok(());
    }

    let info = match frames::load_probe(conn, video_id)? {
        Probe::Info(info) => info,
        Probe::Failed(error) if !force => return Err(error),
        Probe::Unknown | Probe::Failed(_) => {
            let result = probe(path);
            frames::store_probe(conn, video_id, &result)?;
            result?
        }
    };

    for mode in modes {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let key = mode.key();
        if force {
            frames::clear_set(conn, video_id, &key)?;
        } else if frames::set_settled(conn, video_id, &key)? {
            continue;
        }

        let timestamps = mode.timestamps(info.duration_secs);
        let have: HashSet<u32> = frames::cached_indices(conn, video_id, &key)?.into_iter().collect();
        let missing: Vec<u32> = (0..timestamps.len() as u32).filter(|i| !have.contains(i)).collect();
        if missing.is_empty() {
            frames::mark_set(conn, video_id, &key, timestamps.len(), None)?;
            continue;
        }

        let mut first_error: Option<String> = None;
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| -> Result<(), String> {
            let (tx, rx) = mpsc::channel::<(u32, Result<Vec<u8>, String>)>();
            for _ in 0..workers.max(1).min(missing.len()) {
                let tx = tx.clone();
                let (next, missing, timestamps) = (&next, &missing, &timestamps);
                scope.spawn(move || loop {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&idx) = missing.get(i) else { break };
                    if tx.send((idx, extract(path, timestamps[idx as usize]))).is_err() {
                        break;
                    }
                });
            }
            drop(tx);
            for (idx, result) in rx {
                match result {
                    Ok(jpeg) => {
                        frames::store_frame(conn, video_id, &key, idx, timestamps[idx as usize], &jpeg, timestamps.len())?;
                        state.frames_done += 1;
                    }
                    Err(e) => {
                        state.frames_failed += 1;
                        first_error.get_or_insert(e);
                    }
                }
                progress(state);
            }
            Ok(())
        })?;

        if let Some(error) = first_error {
            if !cancel.load(Ordering::SeqCst) {
                frames::mark_set(conn, video_id, &key, timestamps.len(), Some(&error))?;
            }
            state.error(&file.display_name(), &error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::index::sync;
    use crate::cache::test_support::temp_db;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    const NINE: FrameMode = FrameMode::Fixed { count: 9 };
    const MINUTE: FrameMode = FrameMode::Interval { minutes: 1.0 };

    fn info(duration: f64) -> VideoInfo {
        VideoInfo {
            duration_secs: duration,
            file_size_bytes: 0,
            width: None,
            height: None,
            display_aspect_ratio: None,
            codec: None,
            bitrate: None,
            framerate: None,
        }
    }

    fn library() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("one.mp4"), b"one").unwrap();
        fs::write(tmp.path().join("sub/two.mkv"), b"two").unwrap();
        fs::write(tmp.path().join("sub/copy.mkv"), b"two").unwrap();
        fs::write(tmp.path().join("broken.avi"), b"bad").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"txt").unwrap();
        tmp
    }

    fn probe_fake(path: &Path) -> Result<VideoInfo, String> {
        if path.ends_with("broken.avi") {
            Err("not a video".into())
        } else {
            Ok(info(300.0))
        }
    }

    fn frame_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM frames", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn test_generate_all_modes() {
        let (_db, mut conn) = temp_db();
        let tmp = library();
        sync(&mut conn, tmp.path(), false, |_| {}).unwrap();
        let calls = AtomicUsize::new(0);
        let extract = |_: &Path, _: f64| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(b"jpeg".to_vec())
        };
        let cancel = AtomicBool::new(false);
        let mut events = 0;
        let result = generate(&conn, tmp.path(), &[NINE, MINUTE], false, 2, &cancel, &probe_fake, &extract, &mut |_| events += 1).unwrap();

        // one.mp4 and two.mkv (copy.mkv shares two's content): 9 + 4 frames each.
        assert_eq!(result.videos_total, 4);
        assert_eq!(result.videos_done, 4);
        assert_eq!(result.frames_done, 26);
        assert_eq!(calls.load(Ordering::SeqCst), 26);
        assert_eq!(result.videos_failed, 1);
        assert!(result.errors[0].contains("broken.avi"));
        assert!(result.done && !result.cancelled);
        assert!(events > 4);
        assert_eq!(frame_count(&conn), 26);

        // A second run has nothing to do, and does not retry the broken file.
        let probes = AtomicUsize::new(0);
        let probe_counting = |p: &Path| {
            probes.fetch_add(1, Ordering::SeqCst);
            probe_fake(p)
        };
        let again = generate(&conn, tmp.path(), &[NINE, MINUTE], false, 2, &cancel, &probe_counting, &extract, &mut |_| {}).unwrap();
        assert_eq!(again.frames_done, 0);
        assert_eq!(probes.load(Ordering::SeqCst), 0);

        // force regenerates and retries.
        let forced = generate(&conn, tmp.path(), &[NINE], true, 2, &cancel, &probe_counting, &extract, &mut |_| {}).unwrap();
        assert_eq!(forced.frames_done, 18);
        assert_eq!(probes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_failed_frame_marks_set() {
        let (_db, mut conn) = temp_db();
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("one.mp4"), b"one").unwrap();
        sync(&mut conn, tmp.path(), false, |_| {}).unwrap();
        let extract = |_: &Path, t: f64| if t > 200.0 { Err("ffmpeg failed".to_string()) } else { Ok(b"j".to_vec()) };
        let cancel = AtomicBool::new(false);
        let result = generate(&conn, tmp.path(), &[MINUTE], false, 1, &cancel, &probe_fake, &extract, &mut |_| {}).unwrap();
        assert_eq!((result.frames_done, result.frames_failed), (3, 1));
        let error: Option<String> = conn
            .query_row("SELECT error FROM frame_sets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(error.as_deref(), Some("ffmpeg failed"));
    }

    #[test]
    fn test_cancel_stops_job() {
        let (_db, mut conn) = temp_db();
        let tmp = library();
        sync(&mut conn, tmp.path(), false, |_| {}).unwrap();
        let cancel = AtomicBool::new(false);
        let seen = Mutex::new(0);
        let extract = |_: &Path, _: f64| {
            let mut n = seen.lock().unwrap();
            *n += 1;
            if *n == 3 {
                cancel.store(true, Ordering::SeqCst);
            }
            Ok(b"j".to_vec())
        };
        let result = generate(&conn, tmp.path(), &[NINE], false, 1, &cancel, &probe_fake, &extract, &mut |_| {}).unwrap();
        assert!(result.cancelled);
        assert_eq!(result.frames_done, 3);
        // An interrupted set is not marked, so a later run continues it.
        let sets: i64 = conn.query_row("SELECT COUNT(*) FROM frame_sets", [], |r| r.get(0)).unwrap();
        assert_eq!(sets, 0);
    }

    #[test]
    fn test_throttle() {
        let mut throttle = Throttle::new(Duration::from_secs(60));
        assert!(throttle.ready(false));
        assert!(!throttle.ready(false));
        assert!(throttle.ready(true));
        let mut fast = Throttle::new(Duration::ZERO);
        assert!(fast.ready(false) && fast.ready(false));
    }

    #[test]
    fn test_progress_without_errors() {
        let mut progress = GenerateProgress { videos_failed: 1, current: Some("a".into()), ..Default::default() };
        progress.error("a", "broken");
        let light = progress.without_errors();
        assert!(light.errors.is_empty());
        assert_eq!((light.videos_failed, light.current.as_deref()), (1, Some("a")));
    }

    #[test]
    fn test_job_control() {
        let control = JobControl::default();
        let guard = control.start().unwrap();
        assert!(control.start().is_err());
        control.cancel();
        assert!(guard.cancel_flag().load(Ordering::SeqCst));
        drop(guard);
        assert!(!control.is_running());
        let again = control.start().unwrap();
        assert!(!again.cancel_flag().load(Ordering::SeqCst));
    }
}

/// Against the real ffprobe and ffmpeg: `cargo test -- --ignored`.
#[cfg(test)]
mod ffmpeg_tests {
    use super::*;
    use crate::cache::index::{get_entry, path_str, rename_path, sync};
    use crate::cache::test_support::temp_db;
    use crate::commands::fs_commands::encode_node_impl;
    use crate::media;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[ignore]
    fn test_generate_survives_rename_and_encode() {
        let tmp = TempDir::new().unwrap();
        let video = tmp.path().join("clip.mp4");
        let status = media::new_command("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=130:size=160x120:rate=5"])
            .arg(&video)
            .status()
            .unwrap();
        assert!(status.success());

        let (_db, mut conn) = temp_db();
        sync(&mut conn, tmp.path(), false, |_| {}).unwrap();
        let cancel = AtomicBool::new(false);
        let modes = [FrameMode::Fixed { count: 9 }, FrameMode::Interval { minutes: 1.0 }];
        let result = generate(&conn, tmp.path(), &modes, false, 2, &cancel, &media::probe, &media::extract_frame, &mut |_| {}).unwrap();
        assert_eq!(result.frames_done, 11, "{:?}", result.errors);
        let video_id = get_entry(&conn, &path_str(&video)).unwrap().unwrap().video_id.unwrap();
        let first = frames::cached_frames(&conn, video_id, "fixed:9").unwrap();
        assert!(first.iter().all(|(_, _, jpeg)| jpeg.starts_with(&[0xFF, 0xD8])));

        // Rename, then encode the folder it is in, the way the app's commands do.
        let renamed = tmp.path().join("sub");
        fs::create_dir(&renamed).unwrap();
        let moved = renamed.join("clip.mp4");
        fs::rename(&video, &moved).unwrap();
        rename_path(&conn, &video, &moved).unwrap();
        let mut renames = Vec::new();
        encode_node_impl(path_str(&renamed), &mut renames).unwrap();
        for (old, new) in &renames {
            rename_path(&conn, old, new).unwrap();
        }
        let encoded_file = &renames.last().unwrap().1;
        assert_eq!(get_entry(&conn, &path_str(encoded_file)).unwrap().unwrap().video_id, Some(video_id));

        // Nothing left to do: every frame is found under the new path.
        let probes = AtomicUsize::new(0);
        let probe = |p: &Path| {
            probes.fetch_add(1, Ordering::SeqCst);
            media::probe(p)
        };
        let none = |_: &Path, _: f64| -> Result<Vec<u8>, String> { panic!("extracted although cached") };
        let again = generate(&conn, tmp.path(), &modes, false, 2, &cancel, &probe, &none, &mut |_| {}).unwrap();
        assert_eq!((again.frames_done, again.videos_failed), (0, 0));
        assert_eq!(probes.load(Ordering::SeqCst), 0);
    }
}
