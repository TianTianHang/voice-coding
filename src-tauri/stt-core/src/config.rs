#[derive(Debug, Clone)]
pub struct SttConfig {
    pub language: Option<String>,
    pub chunk_seconds: Option<f64>,
    pub max_new_tokens: Option<usize>,
    pub enable_vad: bool,
    pub detect_language: bool,
    pub stream_chunk_seconds: Option<f64>,
    pub stream_unfixed_chunk_num: Option<usize>,
    pub stream_unfixed_token_num: Option<usize>,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            language: None,
            chunk_seconds: Some(30.0),
            max_new_tokens: Some(512),
            enable_vad: true,
            detect_language: false,
            stream_chunk_seconds: None,
            stream_unfixed_chunk_num: None,
            stream_unfixed_token_num: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_batch_transcription_defaults() {
        let config = SttConfig::default();
        assert!(config.language.is_none());
        assert_eq!(config.chunk_seconds, Some(30.0));
        assert_eq!(config.max_new_tokens, Some(512));
        assert!(config.enable_vad);
        assert!(!config.detect_language);
        assert_eq!(config.stream_chunk_seconds, None);
        assert_eq!(config.stream_unfixed_chunk_num, None);
        assert_eq!(config.stream_unfixed_token_num, None);
    }
}
