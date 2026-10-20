use std::path::Path;

use videocut_model::{MediaRef, TICKS_PER_SECOND};

use crate::error::MediaError;
use crate::probe::{file_extension, source_kind, SourceKind};
use crate::proxy::{prelude, FfmpegPlan};
use crate::sidecar::{path_arg, FfmpegSidecar};
use crate::time::format_ticks_as_seconds;

pub const DEFAULT_THUMB_WIDTH: u32 = 320;

/// Poster time: 10% of the file, capped at one second. Stills use time zero.
pub fn poster_ticks(duration_ticks: i64) -> i64 {
    if duration_ticks <= 0 {
        0
    } else {
        (duration_ticks / 10).min(TICKS_PER_SECOND)
    }
}

pub fn plan_thumbnail(
    media: &MediaRef,
    output: &Path,
    at_ticks: i64,
    max_width: u32,
) -> Result<FfmpegPlan, MediaError> {
    if max_width < 2 {
        return Err(MediaError::Invalid(
            "thumbnail width must be at least 2 pixels".into(),
        ));
    }
    match file_extension(output).as_deref() {
        Some("jpg" | "jpeg" | "png") => {}
        _ => {
            return Err(MediaError::OutputExtension {
                preset: "thumbnail",
                expected: "jpg",
                path: output.to_path_buf(),
            });
        }
    }
    if !matches!(source_kind(media)?, SourceKind::Video | SourceKind::Still) {
        return Err(MediaError::NoVideoStream(media.name.clone()));
    }
    let at = if media.duration_ticks <= 0 {
        0
    } else {
        at_ticks.clamp(0, media.duration_ticks.saturating_sub(1))
    };
    let mut args = prelude();
    args.push("-i".into());
    args.push(media.path.clone());
    args.push("-ss".into());
    args.push(format_ticks_as_seconds(at));
    args.extend([
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale=w='min({max_width},iw)':h=-2"),
        path_arg(output),
    ]);
    Ok(FfmpegPlan { args })
}

pub fn write_thumbnail(
    sidecar: &FfmpegSidecar,
    media: &MediaRef,
    output: &Path,
    at_ticks: i64,
    max_width: u32,
) -> Result<std::path::PathBuf, MediaError> {
    let plan = plan_thumbnail(media, output, at_ticks, max_width)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    sidecar.run(&plan.args)?;
    Ok(output.to_path_buf())
}

pub fn write_poster(
    sidecar: &FfmpegSidecar,
    media: &MediaRef,
    output: &Path,
) -> Result<std::path::PathBuf, MediaError> {
    write_thumbnail(
        sidecar,
        media,
        output,
        poster_ticks(media.duration_ticks),
        DEFAULT_THUMB_WIDTH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::ensure_extension;
    use videocut_model::{Rational, VideoStreamInfo, TICKS_PER_SECOND};

    fn clip() -> MediaRef {
        MediaRef {
            id: "media-1".into(),
            name: "clip.mp4".into(),
            path: "clip.mp4".into(),
            duration_ticks: 10 * TICKS_PER_SECOND,
            video: Some(VideoStreamInfo {
                width: 1920,
                height: 1080,
                fps: Rational { num: 30, den: 1 },
                codec: "h264".into(),
            }),
            audio: None,
            proxy_path: None,
        }
    }

    #[test]
    fn poster_uses_one_frame_and_a_bounded_time() {
        assert_eq!(poster_ticks(10 * TICKS_PER_SECOND), TICKS_PER_SECOND);
        let plan = plan_thumbnail(
            &clip(),
            Path::new("poster.jpg"),
            poster_ticks(clip().duration_ticks),
            DEFAULT_THUMB_WIDTH,
        )
        .unwrap();
        assert!(plan.args.iter().any(|arg| arg == "-frames:v"));
        assert!(plan.args.iter().any(|arg| arg == "1"));
        assert!(plan
            .args
            .windows(2)
            .any(|pair| pair[0] == "-ss" && pair[1] == "1"));
        let ss = plan.args.iter().position(|arg| arg == "-ss").unwrap();
        let input = plan.args.iter().position(|arg| arg == "-i").unwrap();
        assert!(input < ss);
    }

    #[test]
    fn ensure_extension_rejects_a_mismatch() {
        let error = ensure_extension(Path::new("out.mov"), "mp4", "proxy").unwrap_err();
        assert!(matches!(
            error,
            MediaError::OutputExtension {
                expected: "mp4",
                ..
            }
        ));
    }
}
