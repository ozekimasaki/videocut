use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ColorError {
    #[error("effect `{0}` is not a color effect")]
    NotColorEffect(String),
    #[error("parameter `{id}` {detail}")]
    InvalidParam { id: String, detail: String },
    #[error("invalid curve: {0}")]
    InvalidCurve(String),
    #[error("invalid cube LUT: {0}")]
    InvalidCube(String),
}
