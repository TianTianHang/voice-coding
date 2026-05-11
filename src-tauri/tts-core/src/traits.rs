use crate::{
    Result, StreamingTextChunk, TtsConfig, TtsError, TtsResult, TtsSynthesisEvent,
};

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

#[async_trait::async_trait]
pub trait StreamingTts: TtsEngine {
    async fn start_stream(
        &self,
        config: TtsConfig,
    ) -> Result<Box<dyn StreamingTtsSession + Send + '_>>;
}

#[async_trait::async_trait]
pub trait StreamingTtsSession: Send {
    async fn push_text(&mut self, chunk: StreamingTextChunk) -> Result<()>;

    async fn next_event(&mut self) -> Result<Option<TtsSynthesisEvent>>;

    async fn finish(&mut self) -> Result<TtsResult>;

    async fn cancel(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioBuffer, PcmData};

    struct BatchOnlyEngine;

    #[async_trait::async_trait]
    impl TtsEngine for BatchOnlyEngine {
        fn engine_name(&self) -> &str {
            "batch-only"
        }

        async fn synthesize(&self, _text: &str, _config: TtsConfig) -> Result<TtsResult> {
            Ok(TtsResult {
                audio: AudioBuffer {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    pcm: PcmData::F32(vec![0.0; 96]),
                },
            })
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn synthesize_stream_defaults_to_unsupported_error() {
        let err = BatchOnlyEngine
            .synthesize_stream("hello", TtsConfig::default())
            .await
            .expect_err("batch-only engine should not support streaming");

        assert!(err.to_string().contains("Streaming synthesis is not supported"));
    }
}
