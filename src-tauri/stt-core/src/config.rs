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
    }
}
