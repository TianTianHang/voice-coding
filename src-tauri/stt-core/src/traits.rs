use crate::{AudioInput, Result, SttConfig, SttError, SttResult, StreamingSttSession};

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

    async fn transcribe_stream(&self, _input: AudioInput, _config: SttConfig) -> Result<SttResult> {
        Err(SttError::NotImplemented(
            "Streaming transcription is not supported by this engine".into(),
        ))
    }

    async fn health_check(&self) -> Result<bool>;
}

#[async_trait::async_trait]
pub trait StreamingStt: SttEngine {
    async fn start_stream(
        &self,
        config: SttConfig,
    ) -> Result<Box<dyn StreamingSttSession + Send + '_>>;
}

pub trait BatchStt: SttEngine {
    fn transcribe_batch_optimized(
        &self,
        inputs: Vec<AudioInput>,
        config: SttConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Result<SttResult>>> + Send + '_>>;
}
