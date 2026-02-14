#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod encoding;
mod models;

use commands::fs_commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            fs_commands::read_directory,
            fs_commands::encode_node,
            fs_commands::decode_node,
            fs_commands::can_encode_node,
            fs_commands::copy_to_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
