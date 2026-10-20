use std::path::Path;

use videocut_model::MediaRef;

use crate::error::MediaError;
use crate::probe::{file_extension, source_kind, SourceKind};
use crate::sidecar::{path_arg, FfmpegSidecar};

/// Half-HD style proxy ceiling used when the caller does not override it.
pub const DEFAULT_PROXY_MAX_WIDTH: u32 = 960;
pub const DEFAULT_PROXY_MAX_HEIGHT: u32 = 540;
pub const DEFAULT_PROXY_CRF: u8 = 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyOptions {
    pub max_width: u32,
    pub max_height: u32,
    pub crf: u8,
}

impl Default for ProxyOptions {
    fn default() -> Self {
        Self {
            max_width: DEFAULT_PROXY_MAX_WIDTH,
            max_height: DEFAULT_PROXY_MAX_HEIGHT,
            crf: DEFAULT_PROXY_CRF,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegPlan {
    pub args: Vec<String>,
}

/// Arguments for a proxy file. Video becomes H.264 MP4, audio-only becomes AAC
/// in M4A, and stills become a scaled JPEG or PNG.
pub fn plan_proxy(
    media: &MediaRef,
    output: &Path,
    options: &ProxyOptions,
) -> Result<FfmpegPlan, MediaError> {
    if options.max_width < 2 || options.max_height < 2 {
        return Err(MediaError::Invalid(
            "proxy size must be at least 2 pixels".into(),
        ));
    }
    if options.crf > 51 {
        return Err(MediaError::Invalid("crf must be 0..=51".into()));
    }
    let kind = source_kind(media)?;
    let expected = expected_proxy_extension(kind, output);
    ensure_extension(output, expected, "proxy")?;
    let mut args = prelude();
    args.push("-i".into());
    args.push(media.path.clone());
    match kind {
        SourceKind::Video => {
            args.push("-vf".into());
            args.push(scale_filter(options.max_width, options.max_height));
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                options.crf.to_string(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
            if media.audio.is_some() {
                args.extend([
                    "-c:a".into(),
                    "aac".into(),
                    "-ac".into(),
                    "2".into(),
                    "-b:a".into(),
                    "128k".into(),
                ]);
            } else {
                args.push("-an".into());
            }
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
        SourceKind::Audio => {
            args.extend([
                "-vn".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "96k".into(),
            ]);
        }
        SourceKind::Still => {
            args.push("-vf".into());
            args.push(scale_filter(options.max_width, options.max_height));
        }
    }
    args.push(path_arg(output));
    Ok(FfmpegPlan { args })
}

pub fn generate_proxy(
    sidecar: &FfmpegSidecar,
    media: &MediaRef,
    output: &Path,
    options: &ProxyOptions,
) -> Result<MediaRef, MediaError> {
    let plan = plan_proxy(media, output, options)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    sidecar.run(&plan.args)?;
    let mut updated = media.clone();
    updated.proxy_path = Some(path_arg(output));
    Ok(updated)
}

fn expected_proxy_extension(kind: SourceKind, output: &Path) -> &'static str {
    match kind {
        SourceKind::Video => "mp4",
        SourceKind::Audio => "m4a",
        SourceKind::Still => match file_extension(output).as_deref() {
            Some("png") => "png",
            Some("jpeg") => "jpeg",
            _ => "jpg",
        },
    }
}

pub(crate) fn ensure_extension(
    path: &Path,
    expected: &'static str,
    preset: &'static str,
) -> Result<(), MediaError> {
    let got = file_extension(path);
    if got.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err(MediaError::OutputExtension {
            preset,
            expected,
            path: path.to_path_buf(),
        })
    }
}

pub(crate) fn prelude() -> Vec<String> {
    ["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn scale_filter(max_width: u32, max_height: u32) -> String {
    format!(
        "scale=w='min({max_width},iw)':h='min({max_height},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{AudioStreamInfo, Rational, VideoStreamInfo};

    fn video_media() -> MediaRef {
        MediaRef {
            id: "media-1".into(),
            name: "clip.mp4".into(),
            path: "C:/My Media/clip.mp4".into(),
            duration_ticks: 1,
            video: Some(VideoStreamInfo {
                width: 1920,
                height: 1080,
                fps: Rational { num: 24, den: 1 },
                codec: "h264".into(),
            }),
            audio: Some(AudioStreamInfo {
                sample_rate: 48_000,
                channels: 2,
                codec: "aac".into(),
            }),
            proxy_path: None,
        }
    }

    #[test]
    fn video_proxy_args_are_one_token_per_path() {
        let plan = plan_proxy(
            &video_media(),
            Path::new("C:/cache/clip.proxy.mp4"),
            &ProxyOptions::default(),
        )
        .unwrap();
        assert!(plan.args.iter().any(|arg| arg == "C:/My Media/clip.mp4"));
        assert!(plan.args.iter().any(|arg| arg.contains("min(960,iw)")));
        assert!(plan.args.iter().any(|arg| arg == "libx264"));
        assert_eq!(
            plan.args.last().map(String::as_str),
            Some("C:/cache/clip.proxy.mp4")
        );
    }

    #[test]
    fn audio_proxy_is_m4a() {
        let mut media = video_media();
        media.video = None;
        media.path = "voice.wav".into();
        let plan = plan_proxy(&media, Path::new("voice.m4a"), &ProxyOptions::default()).unwrap();
        assert!(plan.args.iter().any(|arg| arg == "-vn"));
        assert!(plan.args.iter().any(|arg| arg == "aac"));
    }
}
