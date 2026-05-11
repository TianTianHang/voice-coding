use crate::{Result, TtsConfig, TtsError, TtsResult, TtsSynthesisEvent};

#[async_trait::async_trait]
pub trait TtsEngine: Send + Sync {
    fn engine_name(&self) -> &str;

    async fn synthesize(&self, text: &str, config: TtsConfig) -> Result<TtsResult>;

    async fn synthesize_stream(
        &self,
        _text: &str,
        _config: TtsConfig,
    ) -> Result<Vec<TtsSynthesisEvent>> {
        Err(TtsError::Other(
            "Streaming synthesis is not supported by this engine".to_string(),
        ))
    }

    async fn health_check(&self) -> Result<bool>;
}
