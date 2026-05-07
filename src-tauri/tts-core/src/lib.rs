#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Synthesis failed: {0}")]
    SynthesisError(String),

    #[error("Unsupported configuration: {0}")]
    UnsupportedConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TtsError>;

pub const PLAYBACK_SAMPLE_RATE_HZ: u32 = 48_000;
pub const PLAYBACK_CHANNELS: u16 = 2;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsConfig {
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub pitch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moss: Option<MossTtsConfig>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MossTtsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_audio_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmSampleFormat {
    I16,
    F32,
}

#[derive(Debug, Clone)]
pub enum PcmData {
    I16(Vec<i16>),
    F32(Vec<f32>),
}

impl PcmData {
    pub fn len_frames(&self, channels: u16) -> usize {
        let channels = channels as usize;
        if channels == 0 {
            return 0;
        }
        match self {
            PcmData::I16(samples) => samples.len() / channels,
            PcmData::F32(samples) => samples.len() / channels,
        }
    }

    pub fn sample_format(&self) -> PcmSampleFormat {
        match self {
            PcmData::I16(_) => PcmSampleFormat::I16,
            PcmData::F32(_) => PcmSampleFormat::F32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub pcm: PcmData,
}

impl AudioBuffer {
    pub fn validate(&self) -> Result<()> {
        if self.sample_rate_hz == 0 {
            return Err(TtsError::UnsupportedConfig(
                "sample rate must be greater than zero".to_string(),
            ));
        }
        if self.channels == 0 {
            return Err(TtsError::UnsupportedConfig(
                "channels must be greater than zero".to_string(),
            ));
        }

        let channels = self.channels as usize;
        let len = match &self.pcm {
            PcmData::I16(samples) => samples.len(),
            PcmData::F32(samples) => samples.len(),
        };

        if len % channels != 0 {
            return Err(TtsError::UnsupportedConfig(
                "PCM length must be divisible by channel count".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TtsResult {
    pub audio: AudioBuffer,
}

impl TtsResult {
    pub fn validate_for_playback(&self) -> Result<()> {
        self.audio.validate()?;
        if self.audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ {
            return Err(TtsError::UnsupportedConfig(format!(
                "playback sample rate must be {}Hz",
                PLAYBACK_SAMPLE_RATE_HZ
            )));
        }
        if self.audio.channels != PLAYBACK_CHANNELS {
            return Err(TtsError::UnsupportedConfig(format!(
                "playback channels must be {}",
                PLAYBACK_CHANNELS
            )));
        }
        Ok(())
    }
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
}

#[derive(Debug, Clone)]
pub enum TtsSynthesisEvent {
    Progress(TtsSynthesisProgress),
    AudioChunk(TtsAudioChunk),
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_pcm_channel_alignment() {
        let buffer = AudioBuffer {
            sample_rate_hz: 48_000,
            channels: 2,
            pcm: PcmData::I16(vec![1, 2, 3]),
        };

        let err = buffer.validate().expect_err("must reject misaligned pcm");
        assert!(format!("{err}").contains("divisible"));
    }

    #[test]
    fn validates_playback_constraints() {
        let result = TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(vec![0.0; 480]),
            },
        };

        assert!(result.validate_for_playback().is_ok());
    }
}
