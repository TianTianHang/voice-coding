use serde::{Deserialize, Serialize};

pub const HOP_SIZE: usize = 256;
pub const SAMPLE_RATE: u32 = 16000;
pub const THRESHOLD: f32 = 0.5;
pub const SILENCE_FRAMES: u32 = 30;
pub const MAX_RECORDING_SECONDS: u32 = 30;
pub const MAX_RECORDING_SAMPLES: usize = SAMPLE_RATE as usize * MAX_RECORDING_SECONDS as usize;
pub const MIN_RECORDING_SAMPLES: usize = SAMPLE_RATE as usize / 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VadConfig {
    pub hop_size: usize,
    pub sample_rate: u32,
    pub threshold: f32,
    pub silence_frames: u32,
    pub max_recording_seconds: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            hop_size: HOP_SIZE,
            sample_rate: SAMPLE_RATE,
            threshold: THRESHOLD,
            silence_frames: SILENCE_FRAMES,
            max_recording_seconds: MAX_RECORDING_SECONDS,
        }
    }
}
