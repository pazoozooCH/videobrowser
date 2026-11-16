//! The cache commands through Tauri's IPC, on the mock runtime.


use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};
use tempfile::TempDir;

use crate::cache;
use crate::commands::cache_commands;
use crate::media;

fn request(cmd: &str, body: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
#[ignore]
fn test_generate_cache_answers_and_frees_the_job() {
    let data = TempDir::new().unwrap();
    let library = TempDir::new().unwrap();
    let clip = library.path().join("clip.mp4");
    let ok = media::new_command("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=20:size=160x120:rate=5"])
        .arg(&clip)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    std::fs::write(library.path().join("broken.mp4"), b"not a video").unwrap();
    std::fs::write(library.path().join("empty.mkv"), b"").unwrap();

    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![
            cache_commands::generate_cache,
            cache_commands::cache_job_running,
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(cache::init(data.path()).unwrap());
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();

    let root = library.path().to_string_lossy().to_string();
    let response = get_ipc_response(
        &webview,
        request("generate_cache", serde_json::json!({
            "root": root,
            "modes": [{ "type": "fixed", "count": 9 }],
            "force": false
        })),
    );
    let body = response.expect("generate_cache answered with an error").deserialize::<serde_json::Value>().unwrap();
    assert_eq!(body["framesDone"], 9, "{body}");
    assert_eq!(body["done"], true);

    let running = get_ipc_response(&webview, request("cache_job_running", serde_json::json!({})))
        .unwrap()
        .deserialize::<bool>()
        .unwrap();
    assert!(!running);
}
