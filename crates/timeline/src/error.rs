use thiserror::Error;

/// Failure of a timeline edit. The sequence is left unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EditError {
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("clip not found: {0}")]
    ClipNotFound(String),
    #[error("marker not found: {0}")]
    MarkerNotFound(String),
    #[error("multicam angle not found: {0}")]
    AngleNotFound(String),
    #[error("track is locked: {0}")]
    TrackLocked(String),
    #[error("track is not targeted: {0}")]
    TrackNotTargeted(String),
    #[error("edit has no target")]
    EmptyEdit,
    #[error("range must be positive")]
    InvalidRange,
    #[error("time is outside the sequence")]
    InvalidTime,
    #[error("clip duration must stay positive")]
    ZeroDuration,
    #[error("not enough source media for this edit")]
    InsufficientSource,
    #[error("clips must be adjacent")]
    NotAdjacent,
    #[error("edit would overlap another clip")]
    Overlap,
    #[error("speed must be a finite number greater than zero")]
    InvalidSpeed,
    #[error("playhead is not inside the clip")]
    NotUnderPlayhead,
    #[error("track is already a destination of this edit: {0}")]
    DuplicateTrack(String),
}
