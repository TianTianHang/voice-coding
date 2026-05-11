use crate::{
    AudioBuffer, Result, TtsError, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PcmData;

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
