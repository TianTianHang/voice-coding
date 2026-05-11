use crate::AudioBuffer;

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
}

#[derive(Debug, Clone)]
pub enum TtsSynthesisEvent {
    Progress(TtsSynthesisProgress),
    AudioChunk(TtsAudioChunk),
}
