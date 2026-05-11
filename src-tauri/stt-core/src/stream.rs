use crate::{Result, SttResult};

#[derive(Debug, Clone)]
pub struct StreamingAudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub start_time_sec: Option<f64>,
}

impl StreamingAudioChunk {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
            start_time_sec: None,
        }
    }

    pub fn with_start_time(mut self, start_time_sec: f64) -> Self {
        self.start_time_sec = Some(start_time_sec);
        self
    }

    pub fn duration_sec(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / self.sample_rate as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingTranscript {
    pub text: String,
    pub language: Option<String>,
    pub start_time_sec: Option<f64>,
    pub end_time_sec: Option<f64>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum StreamingSttEvent {
    Partial(StreamingTranscript),
    Final(StreamingTranscript),
    End(SttResult),
}

#[async_trait::async_trait]
pub trait StreamingSttSession: Send {
    async fn push_audio(&mut self, chunk: StreamingAudioChunk) -> Result<()>;

    async fn next_event(&mut self) -> Result<Option<StreamingSttEvent>>;

    async fn finish(&mut self) -> Result<SttResult>;

    async fn cancel(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_audio_chunk_reports_duration() {
        let chunk = StreamingAudioChunk::new(vec![0.0; 8000], 16000).with_start_time(1.5);

        assert_eq!(chunk.duration_sec(), 0.5);
        assert_eq!(chunk.start_time_sec, Some(1.5));
    }

    #[test]
    fn streaming_audio_chunk_handles_zero_sample_rate() {
        let chunk = StreamingAudioChunk::new(vec![0.0; 8000], 0);

        assert_eq!(chunk.duration_sec(), 0.0);
    }
}
