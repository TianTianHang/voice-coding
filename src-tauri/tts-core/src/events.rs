use crate::{AudioBuffer, TtsResult};

#[derive(Debug, Clone)]
pub struct StreamingTextChunk {
    pub text: String,
    pub is_final: bool,
    pub flush: bool,
}

impl StreamingTextChunk {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_final: false,
            flush: false,
        }
    }

    pub fn final_chunk(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_final: true,
            flush: true,
        }
    }

    pub fn with_final(mut self, is_final: bool) -> Self {
        self.is_final = is_final;
        self
    }

    pub fn with_flush(mut self, flush: bool) -> Self {
        self.flush = flush;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TtsSynthesisStarted {
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TtsSynthesisProgress {
    pub stage: String,
    pub produced_chunks: usize,
    pub total_chunks_hint: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct TtsAudioChunk {
    pub sequence: u64,
    pub audio: AudioBuffer,
    pub start_time_sec: Option<f64>,
    pub end_time_sec: Option<f64>,
    pub text_start: Option<usize>,
    pub text_end: Option<usize>,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub struct TtsTextBoundary {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub enum TtsSynthesisEvent {
    Started(TtsSynthesisStarted),
    Progress(TtsSynthesisProgress),
    TextBoundary(TtsTextBoundary),
    AudioChunk(TtsAudioChunk),
    End(TtsResult),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PcmData, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ};

    #[test]
    fn streaming_text_chunk_expresses_incremental_and_final_flush() {
        let incremental = StreamingTextChunk::new("hello");
        assert_eq!(incremental.text, "hello");
        assert!(!incremental.is_final);
        assert!(!incremental.flush);

        let final_chunk = StreamingTextChunk::final_chunk("world");
        assert_eq!(final_chunk.text, "world");
        assert!(final_chunk.is_final);
        assert!(final_chunk.flush);

        let explicit = StreamingTextChunk::new("now")
            .with_final(true)
            .with_flush(true);
        assert!(explicit.is_final);
        assert!(explicit.flush);
    }

    #[test]
    fn tts_audio_chunk_audio_remains_validatable() {
        let chunk = TtsAudioChunk {
            sequence: 7,
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(vec![0.0; PLAYBACK_CHANNELS as usize * 16]),
            },
            start_time_sec: Some(0.0),
            end_time_sec: Some(16.0 / PLAYBACK_SAMPLE_RATE_HZ as f64),
            text_start: Some(0),
            text_end: Some(5),
            is_final: false,
        };

        chunk.audio.validate().expect("chunk audio should be valid");
    }
}
