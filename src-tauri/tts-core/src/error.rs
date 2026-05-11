#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Synthesis failed: {0}")]
    SynthesisError(String),

    #[error("Unsupported configuration: {0}")]
    UnsupportedConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TtsError>;
