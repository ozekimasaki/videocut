use std::path::Path;

use serde_json::Value;
use videocut_model::{new_id, ticks_from_frames, AudioStreamInfo, MediaRef, VideoStreamInfo};

use crate::error::MediaError;
use crate::sidecar::{path_arg, FfmpegSidecar};
use crate::time::{parse_decimal_seconds_to_ticks, parse_frame_rate, seconds_f64_to_ticks};

/// Extensions accepted by [`import_media`]. `jpeg` is the long form of `jpg`.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "mxf", "wav", "aac", "mp3", "png", "jpg", "jpeg",
];

const STILL_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Video,
    Audio,
    Still,
}

pub fn supported_extensions() -> &'static [&'static str] {
    SUPPORTED_EXTENSIONS
}

pub fn file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn is_supported_extension(path: &Path) -> bool {
    file_extension(path).is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
}

pub fn is_still_extension(path: &Path) -> bool {
    file_extension(path).is_some_and(|ext| STILL_EXTENSIONS.contains(&ext.as_str()))
}

pub fn ensure_supported_extension(path: &Path) -> Result<(), MediaError> {
    if is_supported_extension(path) {
        Ok(())
    } else {
        Err(MediaError::UnsupportedExtension {
            extension: file_extension(path).unwrap_or_else(|| "(none)".to_string()),
        })
    }
}

pub(crate) fn source_kind(media: &MediaRef) -> Result<SourceKind, MediaError> {
    let still = is_still_extension(Path::new(&media.path))
        || (media.video.is_some() && media.audio.is_none() && media.duration_ticks == 0);
    if still {
        if media.video.is_none() {
            return Err(MediaError::NoVideoStream(media.path.clone()));
        }
        return Ok(SourceKind::Still);
    }
    if media.video.is_some() {
        return Ok(SourceKind::Video);
    }
    if media.audio.is_some() {
        return Ok(SourceKind::Audio);
    }
    Err(MediaError::Invalid(format!(
        "media '{}' has no audio or video stream",
        media.name
    )))
}

/// Probe a supported file and build a [`MediaRef`].
pub fn import_media(sidecar: &FfmpegSidecar, path: &Path) -> Result<MediaRef, MediaError> {
    ensure_supported_extension(path)?;
    if !path.is_file() {
        return Err(MediaError::NotFound {
            path: path_arg(path),
        });
    }
    let json = sidecar.probe_json(path)?;
    media_ref_from_probe_json(path, &json)
}

/// Build a [`MediaRef`] from ffprobe's JSON (`-show_format -show_streams`).
///
/// Still images (`png`, `jpg`, `jpeg`) get `duration_ticks = 0`. Their timeline
/// length is decided when a clip is cut, not by the file.
pub fn media_ref_from_probe_json(path: &Path, json: &str) -> Result<MediaRef, MediaError> {
    let value: Value = serde_json::from_str(json).map_err(|error| MediaError::ProbeFailed {
        message: format!("invalid ffprobe json: {error}"),
    })?;
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let format = value.get("format");

    let video_stream = streams.iter().find(|stream| codec_type_is(stream, "video"));
    let audio_stream = streams.iter().find(|stream| codec_type_is(stream, "audio"));
    let still = is_still_extension(path);

    let video = match video_stream {
        Some(stream) => Some(video_info(stream, still)?),
        None => None,
    };
    let audio = match audio_stream {
        Some(stream) => Some(audio_info(stream)?),
        None => None,
    };
    if video.is_none() && audio.is_none() {
        return Err(MediaError::ProbeFailed {
            message: format!("{} has no audio or video stream", path_arg(path)),
        });
    }

    let duration_ticks = if still {
        0
    } else {
        duration_ticks(format, video_stream, audio_stream, video.as_ref())?
    };

    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "media".to_string());

    Ok(MediaRef {
        id: new_id("media"),
        name,
        path: path_arg(path),
        duration_ticks,
        video,
        audio,
        proxy_path: None,
    })
}

fn video_info(stream: &Value, still: bool) -> Result<VideoStreamInfo, MediaError> {
    let width = field_u32(stream, "width").ok_or_else(|| MediaError::ProbeFailed {
        message: "video stream is missing width".into(),
    })?;
    let height = field_u32(stream, "height").ok_or_else(|| MediaError::ProbeFailed {
        message: "video stream is missing height".into(),
    })?;
    let fps = field_str(stream, "avg_frame_rate")
        .and_then(|text| parse_frame_rate(text).ok())
        .or_else(|| field_str(stream, "r_frame_rate").and_then(|text| parse_frame_rate(text).ok()));
    let fps = match fps {
        Some(fps) => fps,
        None if still => videocut_model::Rational { num: 1, den: 1 },
        None => return Err(MediaError::InvalidFrameRate),
    };
    let codec = field_str(stream, "codec_name")
        .unwrap_or("unknown")
        .to_string();
    Ok(VideoStreamInfo {
        width,
        height,
        fps,
        codec,
    })
}

fn audio_info(stream: &Value) -> Result<AudioStreamInfo, MediaError> {
    let sample_rate = field_u32(stream, "sample_rate").ok_or_else(|| MediaError::ProbeFailed {
        message: "audio stream is missing sample_rate".into(),
    })?;
    let channels = field_u32(stream, "channels")
        .and_then(|value| u16::try_from(value).ok())
        .or_else(|| {
            field_str(stream, "channel_layout")
                .and_then(channels_from_layout)
                .or_else(|| field_str(stream, "ch_layout").and_then(channels_from_layout))
        })
        .ok_or_else(|| MediaError::ProbeFailed {
            message: "audio stream is missing channels".into(),
        })?;
    let codec = field_str(stream, "codec_name")
        .unwrap_or("unknown")
        .to_string();
    Ok(AudioStreamInfo {
        sample_rate,
        channels,
        codec,
    })
}

fn duration_ticks(
    format: Option<&Value>,
    video_stream: Option<&Value>,
    audio_stream: Option<&Value>,
    video: Option<&VideoStreamInfo>,
) -> Result<i64, MediaError> {
    if let Some(ticks) = format.and_then(|value| field_duration(value, "duration")) {
        return Ok(ticks);
    }
    for stream in [video_stream, audio_stream].into_iter().flatten() {
        if let Some(ticks) = field_duration(stream, "duration") {
            return Ok(ticks);
        }
    }
    if let (Some(video), Some(stream)) = (video, video_stream) {
        if let Some(frames) = field_i64(stream, "nb_frames") {
            if frames > 0 {
                return ticks_from_frames(frames, video.fps)
                    .map_err(|_| MediaError::InvalidFrameRate);
            }
        }
    }
    Err(MediaError::ProbeFailed {
        message: "media duration is missing".into(),
    })
}

fn codec_type_is(stream: &Value, expected: &str) -> bool {
    field_str(stream, "codec_type").is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn field_u32(value: &Value, key: &str) -> Option<u32> {
    let field = value.get(key)?;
    if let Some(number) = field.as_u64() {
        return u32::try_from(number).ok();
    }
    if let Some(text) = field.as_str() {
        return text.parse().ok();
    }
    None
}

fn field_i64(value: &Value, key: &str) -> Option<i64> {
    let field = value.get(key)?;
    if let Some(number) = field.as_i64() {
        return Some(number);
    }
    if let Some(text) = field.as_str() {
        return text.parse().ok();
    }
    None
}

fn field_duration(value: &Value, key: &str) -> Option<i64> {
    let field = value.get(key)?;
    if let Some(number) = field.as_f64() {
        return seconds_f64_to_ticks(number).ok();
    }
    if let Some(text) = field.as_str() {
        return parse_decimal_seconds_to_ticks(text).ok();
    }
    None
}

fn channels_from_layout(layout: &str) -> Option<u16> {
    match layout {
        "mono" => Some(1),
        "stereo" => Some(2),
        "2.1" => Some(3),
        "5.1" | "5.1(side)" => Some(6),
        "7.1" => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::TICKS_PER_SECOND;

    const MP4: &str = r#"{
      "streams": [
        {
          "codec_name": "h264",
          "codec_type": "video",
          "width": 1920,
          "height": 1080,
          "avg_frame_rate": "30000/1001",
          "r_frame_rate": "30000/1001",
          "duration": "10.0"
        },
        {
          "codec_name": "aac",
          "codec_type": "audio",
          "sample_rate": "48000",
          "channels": 2,
          "duration": "10.0"
        }
      ],
      "format": { "duration": "10.0", "format_name": "mov,mp4,m4a,3gp,3g2,mj2" }
    }"#;

    const WAV: &str = r#"{
      "streams": [
        {
          "codec_name": "pcm_s16le",
          "codec_type": "audio",
          "sample_rate": "48000",
          "channel_layout": "mono",
          "duration": "2.5"
        }
      ],
      "format": { "duration": "2.5" }
    }"#;

    const PNG: &str = r#"{
      "streams": [
        {
          "codec_name": "png",
          "codec_type": "video",
          "width": 1280,
          "height": 720,
          "avg_frame_rate": "25/1"
        }
      ],
      "format": { "format_name": "image2" }
    }"#;

    #[test]
    fn mp4_probe_fills_picture_sound_and_duration() {
        let media = media_ref_from_probe_json(Path::new("C:/media/clip.mp4"), MP4).unwrap();
        assert!(media.id.starts_with("media-"));
        assert_eq!(media.name, "clip.mp4");
        assert_eq!(media.duration_ticks, 10 * TICKS_PER_SECOND);
        let video = media.video.clone().unwrap();
        assert_eq!(video.width, 1920);
        assert_eq!(video.height, 1080);
        assert_eq!(video.fps.num, 30000);
        assert_eq!(video.fps.den, 1001);
        assert_eq!(video.codec, "h264");
        let audio = media.audio.clone().unwrap();
        assert_eq!(audio.sample_rate, 48_000);
        assert_eq!(audio.channels, 2);
        assert_eq!(audio.codec, "aac");
        assert!(media.proxy_path.is_none());
        let json = serde_json::to_value(&media).unwrap();
        assert!(json.get("durationTicks").is_some());
        assert!(json.get("proxyPath").is_none());
    }

    #[test]
    fn wav_probe_is_audio_only() {
        let media = media_ref_from_probe_json(Path::new("voice.wav"), WAV).unwrap();
        assert!(media.video.is_none());
        assert_eq!(media.audio.unwrap().channels, 1);
        assert_eq!(media.duration_ticks, TICKS_PER_SECOND * 5 / 2);
    }

    #[test]
    fn png_probe_is_a_still_with_zero_duration() {
        let media = media_ref_from_probe_json(Path::new("title.png"), PNG).unwrap();
        assert_eq!(media.duration_ticks, 0);
        assert_eq!(media.video.as_ref().unwrap().width, 1280);
        assert!(media.audio.is_none());
        assert_eq!(source_kind(&media).unwrap(), SourceKind::Still);
    }

    #[test]
    fn supported_extensions_cover_the_import_set() {
        for ext in [
            "mp4", "mov", "mxf", "wav", "aac", "mp3", "png", "jpg", "jpeg", "MP4",
        ] {
            assert!(is_supported_extension(Path::new(&format!("clip.{ext}"))));
        }
        let error = ensure_supported_extension(Path::new("notes.txt")).unwrap_err();
        assert!(matches!(
            error,
            MediaError::UnsupportedExtension { extension } if extension == "txt"
        ));
    }
}
