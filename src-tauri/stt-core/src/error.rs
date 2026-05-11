#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("Audio load error: {0}")]
    AudioLoadError(String),

    #[error("Inference error in {model}: {detail}")]
    InferenceError { model: String, detail: String },

    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SttError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_domain_errors() {
        let err = SttError::AudioLoadError("file not found".into());
        assert_eq!(format!("{}", err), "Audio load error: file not found");

        let err = SttError::InferenceError {
            model: "encoder_conv".into(),
            detail: "shape mismatch".into(),
        };
        assert!(format!("{}", err).contains("encoder_conv"));
        assert!(format!("{}", err).contains("shape mismatch"));

        let err = SttError::UnsupportedLanguage("xx".into());
        assert!(format!("{}", err).contains("xx"));
    }

    #[test]
    fn converts_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        let stt_err: SttError = io_err.into();
        match stt_err {
            SttError::Io(_) => {}
            other => panic!("Expected Io variant, got {:?}", other),
        }
    }
}
