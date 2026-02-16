#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cache;
mod commands;
mod encoding;
mod models;

use tauri::Manager;

use commands::fs_commands;
use commands::video_commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let cache_state = cache::init_db(&data_dir)
                .expect("failed to initialize frame cache");
            app.manage(cache_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fs_commands::read_directory,
            fs_commands::encode_node,
            fs_commands::decode_node,
            fs_commands::can_encode_node,
            fs_commands::copy_to_clipboard,
            fs_commands::rename_node,
            fs_commands::delete_node,
            fs_commands::count_children,
            fs_commands::move_node,
            fs_commands::show_in_file_manager,
            fs_commands::open_in_vlc,
            fs_commands::get_cli_path,
            fs_commands::search_files,
            video_commands::list_video_files,
            video_commands::get_video_info,
            video_commands::extract_video_frame,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
