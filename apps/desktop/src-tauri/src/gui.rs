use tauri::Manager;

use crate::bridge::{self, ExportOutcome, LoadedProject};
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

#[tauri::command]
fn read_prproj(path: String) -> Result<LoadedProject, String> {
    bridge::read_prproj(&path)
}

#[tauri::command]
fn write_prproj(path: String, project_json: String) -> Result<Vec<String>, String> {
    bridge::write_prproj(&path, &project_json)
}

#[tauri::command]
fn import_interchange(path: String, format: String) -> Result<LoadedProject, String> {
    bridge::import_interchange(&path, &format)
}

#[tauri::command]
fn export_interchange(
    path: String,
    project_json: String,
    format: String,
    sequence_id: Option<String>,
) -> Result<Vec<String>, String> {
    bridge::export_interchange(&path, &project_json, &format, sequence_id.as_deref())
}

#[tauri::command]
fn probe_media(path: String) -> Result<String, String> {
    bridge::probe_media(&path)
}

#[tauri::command]
fn export_media(project_json: String, request_json: String) -> Result<ExportOutcome, String> {
    bridge::export_media(&project_json, &request_json)
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
            export_status,
            read_prproj,
            write_prproj,
            import_interchange,
            export_interchange,
            probe_media,
            export_media
        ])
        .run(tauri::generate_context!())
        .expect("VideoCut の起動に失敗しました");
}
