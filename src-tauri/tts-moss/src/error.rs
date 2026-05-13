#[derive(Debug, thiserror::Error)]
pub enum MossTtsError {
    #[error("missing required MOSS asset ({kind}): {path}")]
    MissingFile { kind: &'static str, path: PathBuf },

    #[error("failed to parse MOSS asset {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("invalid relative path for {field}: {raw}")]
    InvalidRelativePath { field: String, raw: String },

    #[error("MOSS metadata mismatch: {0}")]
    MetadataMismatch(String),

    #[error("MOSS tokenizer error: {0}")]
    Tokenizer(String),

    #[error("MOSS inference failed at {stage}: {detail}")]
    Inference { stage: &'static str, detail: String },

    #[error("unknown MOSS voice '{voice}'. Available voices include: {available}")]
    UnknownVoice { voice: String, available: String },

    #[error("unknown MOSS sampling mode '{mode}'. Available modes: fixed")]
    UnknownSamplingMode { mode: String },

    #[error("MOSS output format error: {0}")]
    OutputFormat(String),
}

impl From<MossTtsError> for TtsError {
    fn from(value: MossTtsError) -> Self {
        match value {
            MossTtsError::MissingFile { .. }
            | MossTtsError::Parse { .. }
            | MossTtsError::InvalidRelativePath { .. }
            | MossTtsError::MetadataMismatch(_) => TtsError::UnsupportedConfig(value.to_string()),
            MossTtsError::Tokenizer(_) => TtsError::SynthesisError(value.to_string()),
            MossTtsError::Inference { .. } => TtsError::SynthesisError(value.to_string()),
            MossTtsError::UnknownVoice { .. } | MossTtsError::UnknownSamplingMode { .. } => {
                TtsError::UnsupportedConfig(value.to_string())
            }
            MossTtsError::OutputFormat(_) => TtsError::UnsupportedConfig(value.to_string()),
        }
    }
}
