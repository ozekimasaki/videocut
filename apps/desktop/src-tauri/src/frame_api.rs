use serde::Serialize;

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
    let _ = (path, time_ticks, width, height);
    FrameResponse {
        available: false,
        reason: if cfg!(videocut_media_present) {
            "present-unlinked".into()
        } else {
            "absent".into()
        },
        width: 0,
        height: 0,
        png_base64: None,
    }
}

pub fn export_status_message() -> String {
    if cfg!(videocut_media_present) {
        "メディアクレートは見つかりましたが、書き出し API はまだシェルに接続されていません".into()
    } else {
        "メディアクレートが無いため、書き出しはまだ実行できません".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_api_stays_unavailable_until_media_is_linked() {
        let frame = frame_at("clip.mp4", 0, 320, 180);
        assert!(!frame.available);
        assert!(frame.png_base64.is_none());
        assert!(frame.reason == "absent" || frame.reason == "present-unlinked");
    }
}
