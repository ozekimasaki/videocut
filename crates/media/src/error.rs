use std::io;
use std::path::PathBuf;

/// Failures from ingest, proxy generation, waveforms, and export.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("unsupported media extension '{extension}'")]
    UnsupportedExtension { extension: String },

    #[error(
        "ffmpeg sidecar was not found. Set VIDEOCUT_FFMPEG and VIDEOCUT_FFPROBE, set VIDEOCUT_FFMPEG_DIR, place ffmpeg and ffprobe next to the app, or install them on PATH"
    )]
    SidecarNotFound,

    #[error("media file does not exist: {path}")]
    NotFound { path: String },

    #[error("ffprobe failed: {message}")]
    ProbeFailed { message: String },

    #[error("ffmpeg failed: {message}")]
    FfmpegFailed { message: String },

    #[error("encoder '{name}' is not available in this ffmpeg build")]
    EncoderUnavailable { name: String },

    #[error("sequence '{0}' was not found")]
    SequenceNotFound(String),

    #[error("media '{0}' was not found")]
    MediaNotFound(String),

    #[error("no video stream in '{0}'")]
    NoVideoStream(String),

    #[error("no audio stream in '{0}'")]
    NoAudioStream(String),

    #[error("timeline is empty")]
    EmptyTimeline,

    #[error("export range is empty")]
    EmptyRange,

    #[error("{preset} output must use the .{expected} extension (got {path})")]
    OutputExtension {
        preset: &'static str,
        expected: &'static str,
        path: PathBuf,
    },

    #[error("{0}")]
    Invalid(String),

    #[error("frame rate must be a positive rational")]
    InvalidFrameRate,

    #[error(transparent)]
    Io(#[from] io::Error),
}
