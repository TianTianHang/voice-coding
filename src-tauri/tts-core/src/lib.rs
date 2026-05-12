mod audio;
mod config;
mod error;
mod events;
mod result;
mod traits;

pub use audio::{
    AudioBuffer, PcmData, PcmSampleFormat, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ,
};
pub use config::{MossTtsConfig, PlaybackBufferConfig, TtsConfig, TtsStreamConfig};
pub use error::{Result, TtsError};
pub use events::{
    StreamingTextChunk, TtsAudioChunk, TtsSynthesisEvent, TtsSynthesisProgress,
    TtsSynthesisStarted, TtsTextBoundary,
};
pub use result::TtsResult;
pub use traits::{StreamingTts, StreamingTtsSession, TtsEngine};
