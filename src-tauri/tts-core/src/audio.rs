use crate::{Result, TtsError};

pub const PLAYBACK_SAMPLE_RATE_HZ: u32 = 48_000;
pub const PLAYBACK_CHANNELS: u16 = 2;

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
}
