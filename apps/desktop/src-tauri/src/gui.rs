use tauri::Manager;

use crate::frame_api::{self, FrameResponse};
use crate::project_store;

#[tauri::command]
fn read_project(path: String) -> Result<String, String> {
    project_store::read_project_json(&path)
}

#[tauri::command]
fn write_project(path: String, project_json: String) -> Result<(), String> {
    project_store::write_project_json(&path, &project_json)
}

#[tauri::command]
fn frame_at(path: String, time_ticks: i64, width: u32, height: u32) -> FrameResponse {
    frame_api::frame_at(&path, time_ticks, width, height)
}

#[tauri::command]
fn export_status() -> String {
    frame_api::export_status_message()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("VideoCut");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_project,
            write_project,
            frame_at,
            export_status
        ])
        .run(tauri::generate_context!())
        .expect("VideoCut の起動に失敗しました");
}
