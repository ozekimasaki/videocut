use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("OpenTimelineIO JSON could not be read: {0}")]
    Otio(String),
    #[error("FCPXML could not be read: {0}")]
    Fcpxml(String),
    #[error("CMX 3600 EDL could not be read: {0}")]
    Edl(String),
    #[error("the project has no sequence to export")]
    NoSequence,
    #[error("sequence '{0}' was not found")]
    SequenceNotFound(String),
}
