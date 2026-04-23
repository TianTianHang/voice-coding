#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("Audio load error: {0}")]
    AudioLoadError(String),

    #[error("Inference error in {model}: {detail}")]
    InferenceError {
        model: String,
        detail: String,
    },

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

#[derive(Debug, Clone)]
pub enum AudioInput {
    FilePath(String),
    Bytes(Vec<u8>),
    Samples(Vec<f32>, u32),
}

#[derive(Debug, Clone)]
pub struct SttConfig {
    pub language: Option<String>,
    pub chunk_seconds: Option<f64>,
    pub max_new_tokens: Option<usize>,
    pub enable_vad: bool,
    pub detect_language: bool,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            language: None,
            chunk_seconds: Some(30.0),
            max_new_tokens: Some(512),
            enable_vad: true,
            detect_language: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimingInfo {
    pub audio_duration_sec: f64,
    pub processing_time_sec: f64,
    pub rtf: f64,
    pub tokens_generated: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SttResult {
    pub text: String,
    pub language: String,
    pub confidence: Option<f64>,
    pub timing: TimingInfo,
}

#[async_trait::async_trait]
pub trait SttEngine: Send + Sync {
    fn engine_name(&self) -> &str;

    fn supported_languages(&self) -> &[&str];

    async fn transcribe(&self, input: AudioInput, config: SttConfig) -> Result<SttResult>;

    async fn transcribe_batch(
        &self,
        inputs: Vec<AudioInput>,
        config: SttConfig,
    ) -> Vec<Result<SttResult>> {
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.transcribe(input, config.clone()).await);
        }
        results
    }

    async fn transcribe_stream(
        &self,
        _input: AudioInput,
        _config: SttConfig,
    ) -> Result<SttResult> {
        Err(SttError::NotImplemented(
            "Streaming transcription is not supported by this engine".into(),
        ))
    }

    async fn health_check(&self) -> Result<bool>;
}

pub trait StreamingStt: SttEngine {}

pub trait BatchStt: SttEngine {
    fn transcribe_batch_optimized(
        &self,
        inputs: Vec<AudioInput>,
        config: SttConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Result<SttResult>>> + Send + '_>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_config_default() {
        let config = SttConfig::default();
        assert!(config.language.is_none());
        assert_eq!(config.chunk_seconds, Some(30.0));
        assert_eq!(config.max_new_tokens, Some(512));
        assert!(config.enable_vad);
        assert!(!config.detect_language);
    }

    #[test]
    fn test_audio_input_variants() {
        let file_input = AudioInput::FilePath("/path/to/audio.wav".into());
        let bytes_input = AudioInput::Bytes(vec![0u8; 1024]);
        let samples_input = AudioInput::Samples(vec![0.5f32; 16000], 16000);

        match file_input {
            AudioInput::FilePath(p) => assert_eq!(p, "/path/to/audio.wav"),
            _ => panic!("Expected FilePath variant"),
        }
        match bytes_input {
            AudioInput::Bytes(b) => assert_eq!(b.len(), 1024),
            _ => panic!("Expected Bytes variant"),
        }
        match samples_input {
            AudioInput::Samples(s, r) => {
                assert_eq!(s.len(), 16000);
                assert_eq!(r, 16000);
            }
            _ => panic!("Expected Samples variant"),
        }
    }

    #[test]
    fn test_stt_error_display() {
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
    fn test_stt_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        let stt_err: SttError = io_err.into();
        match stt_err {
            SttError::Io(_) => {}
            other => panic!("Expected Io variant, got {:?}", other),
        }
    }

    #[test]
    fn test_timing_info() {
        let timing = TimingInfo {
            audio_duration_sec: 10.0,
            processing_time_sec: 3.2,
            rtf: 0.32,
            tokens_generated: Some(45),
        };
        assert_eq!(timing.rtf, 0.32);
        assert!(timing.rtf < 1.0);
    }
}
