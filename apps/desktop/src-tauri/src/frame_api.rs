use std::fs;
use std::path::Path;

use base64::Engine;
use serde::Serialize;
use videocut_media::{import_media, write_thumbnail, FfmpegSidecar};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameResponse {
    pub available: bool,
    pub reason: String,
    pub width: u32,
    pub height: u32,
    pub png_base64: Option<String>,
}

pub fn frame_at(path: &str, time_ticks: i64, width: u32, height: u32) -> FrameResponse {
    match grab_png(path, time_ticks, width.max(2)) {
        Ok(bytes) => FrameResponse {
            available: true,
            reason: String::new(),
            width,
            height,
            png_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
        },
        Err(reason) => FrameResponse {
            available: false,
            reason,
            width: 0,
            height: 0,
            png_base64: None,
        },
    }
}

pub fn export_status_message() -> String {
    match FfmpegSidecar::locate() {
        Ok(sidecar) => format!(
            "FFmpeg が見つかりました（{}）。シーケンスを書き出せます",
            sidecar.ffmpeg.display()
        ),
        Err(_) => "FFmpeg が見つからないため、書き出しはまだ実行できません".into(),
    }
}

fn grab_png(path: &str, time_ticks: i64, width: u32) -> Result<Vec<u8>, String> {
    let sidecar = FfmpegSidecar::locate().map_err(|error| error.to_string())?;
    let media = import_media(&sidecar, Path::new(path)).map_err(|error| error.to_string())?;
    let dir = std::env::temp_dir().join(format!("videocut-frame-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let output = dir.join("frame.png");
    write_thumbnail(&sidecar, &media, &output, time_ticks, width)
        .map_err(|error| error.to_string())?;
    let bytes = fs::read(&output).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(&dir);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_media_does_not_invent_a_frame() {
        let frame = frame_at("clip.mp4", 0, 320, 180);
        assert!(!frame.available);
        assert!(frame.png_base64.is_none());
        assert!(!frame.reason.is_empty());
    }
}
