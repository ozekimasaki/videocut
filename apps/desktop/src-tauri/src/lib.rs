mod bridge;
mod frame_api;
mod project_store;

#[cfg(feature = "gui")]
mod gui;

pub use bridge::{
    export_interchange, export_media, import_interchange, probe_media, read_prproj, write_prproj,
    ExportOutcome, LoadedProject,
};
pub use frame_api::{export_status_message, frame_at, FrameResponse};
pub use project_store::{read_project_json, write_project_json};

pub fn run() {
    #[cfg(feature = "gui")]
    {
        gui::run();
        return;
    }
    #[cfg(not(feature = "gui"))]
    {
        eprintln!("VideoCut のウィンドウを開くには --features gui が必要です");
        std::process::exit(1);
    }
}
