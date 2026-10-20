use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphicsError {
    #[error("range end must be after start")]
    InvalidRange,
    #[error("video track was not found")]
    TrackNotFound,
    #[error("graphics clips belong on a video track")]
    NotVideoTrack,
    #[error("track is locked")]
    TrackLocked,
    #[error("clip is not a title")]
    NotTitle,
    #[error("clip is not a lower third")]
    NotLowerThird,
    #[error("caption {0} was not found")]
    CaptionNotFound(String),
    #[error("graphic parameter `{0}` is missing or has the wrong type")]
    InvalidGraphicParam(&'static str),
    #[error("font size must be positive")]
    InvalidFontSize,
    #[error("opacity must be between 0 and 1")]
    InvalidOpacity,
    #[error("position must be finite")]
    InvalidPosition,
    #[error("bar height must be between 0 and 1")]
    InvalidBarHeight,
    #[error("subtitle parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("transcription failed: {0}")]
    Transcription(String),
}
