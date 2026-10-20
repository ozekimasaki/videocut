use videocut_model::{effect_ids, ClipKind, MarkerKind, TrackKind, TransitionAlign};

pub fn clip_kind_name(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Media => "media",
        ClipKind::Nested => "nested",
        ClipKind::Adjustment => "adjustment",
        ClipKind::Title => "title",
        ClipKind::ColorMatte => "colorMatte",
        ClipKind::Caption => "caption",
    }
}

pub fn parse_clip_kind(text: &str) -> ClipKind {
    match text {
        "nested" => ClipKind::Nested,
        "adjustment" => ClipKind::Adjustment,
        "title" => ClipKind::Title,
        "colorMatte" => ClipKind::ColorMatte,
        "caption" => ClipKind::Caption,
        _ => ClipKind::Media,
    }
}

pub fn marker_kind_name(kind: MarkerKind) -> &'static str {
    match kind {
        MarkerKind::Comment => "comment",
        MarkerKind::Chapter => "chapter",
        MarkerKind::Segmentation => "segmentation",
        MarkerKind::Caption => "caption",
    }
}

pub fn parse_marker_kind(text: &str) -> MarkerKind {
    match text {
        "chapter" => MarkerKind::Chapter,
        "segmentation" => MarkerKind::Segmentation,
        "caption" => MarkerKind::Caption,
        _ => MarkerKind::Comment,
    }
}

pub fn align_name(align: TransitionAlign) -> &'static str {
    match align {
        TransitionAlign::Start => "start",
        TransitionAlign::Center => "center",
        TransitionAlign::End => "end",
    }
}

pub fn parse_align(text: &str) -> TransitionAlign {
    match text {
        "start" => TransitionAlign::Start,
        "end" => TransitionAlign::End,
        _ => TransitionAlign::Center,
    }
}

pub fn track_kind_name(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "video",
        TrackKind::Audio => "audio",
    }
}

pub fn effect_label(effect_id: &str) -> &'static str {
    match effect_id {
        effect_ids::CROSS_DISSOLVE => "Cross Dissolve",
        effect_ids::DIP_TO_BLACK => "Dip to Black",
        effect_ids::DIP_TO_WHITE => "Dip to White",
        effect_ids::WIPE => "Wipe",
        _ => "Cross Dissolve",
    }
}

pub fn effect_from_label(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if lower.contains("dip") && lower.contains("white") {
        effect_ids::DIP_TO_WHITE.to_string()
    } else if lower.contains("dip") && lower.contains("black") {
        effect_ids::DIP_TO_BLACK.to_string()
    } else if lower.contains("wipe") {
        effect_ids::WIPE.to_string()
    } else if text.starts_with("builtin.") {
        text.to_string()
    } else {
        effect_ids::CROSS_DISSOLVE.to_string()
    }
}

pub fn otio_transition_type(effect_id: &str) -> &'static str {
    match effect_id {
        effect_ids::WIPE => "SMPTE_Wipe",
        effect_ids::DIP_TO_BLACK => "Custom",
        effect_ids::DIP_TO_WHITE => "Custom",
        _ => "SMPTE_Dissolve",
    }
}

pub fn effect_from_otio(transition_type: &str, metadata_effect: Option<&str>) -> String {
    if let Some(effect) = metadata_effect {
        if effect.starts_with("builtin.") {
            return effect.to_string();
        }
    }
    match transition_type {
        "SMPTE_Wipe" => effect_ids::WIPE.to_string(),
        "SMPTE_Dissolve" => effect_ids::CROSS_DISSOLVE.to_string(),
        other => effect_from_label(other),
    }
}

const MARKER_COLORS: [&str; 8] = [
    "RED", "ORANGE", "YELLOW", "GREEN", "CYAN", "BLUE", "PURPLE", "PINK",
];

pub fn marker_color_name(index: u8) -> &'static str {
    MARKER_COLORS[(index as usize) % MARKER_COLORS.len()]
}

pub fn marker_color_index(name: &str) -> u8 {
    MARKER_COLORS
        .iter()
        .position(|color| color.eq_ignore_ascii_case(name))
        .unwrap_or(0) as u8
}

pub fn file_url(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let encoded = percent_encode(&normalized);
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

pub fn path_from_url(url: &str) -> String {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://"))
        .unwrap_or(url);
    let decoded = percent_decode(rest);
    if decoded.len() >= 3 && decoded.as_bytes()[0] == b'/' && decoded.as_bytes()[2] == b':' {
        decoded[1..].to_string()
    } else {
        decoded
    }
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn single_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}
