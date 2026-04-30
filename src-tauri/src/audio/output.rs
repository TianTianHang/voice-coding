use std::collections::VecDeque;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use tts_core::{AudioBuffer, PcmData, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ};

#[derive(Debug)]
pub enum AudioOutputError {
    NoOutputDevice,
    StreamConfig(String),
    StreamBuild(String),
    StreamPlay(String),
}

impl std::fmt::Display for AudioOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioOutputError::NoOutputDevice => write!(f, "No output device found"),
            AudioOutputError::StreamConfig(s) => write!(f, "Stream config error: {s}"),
            AudioOutputError::StreamBuild(s) => write!(f, "Stream build error: {s}"),
            AudioOutputError::StreamPlay(s) => write!(f, "Stream play error: {s}"),
        }
    }
}

impl std::error::Error for AudioOutputError {}

#[derive(Debug, Clone)]
pub struct PlaybackBuffer {
    samples: Arc<Vec<f32>>,
}

impl PlaybackBuffer {
    pub fn from_samples(samples: Vec<f32>) -> Self {
        Self {
            samples: Arc::new(samples),
        }
    }

    pub fn samples(&self) -> Arc<Vec<f32>> {
        self.samples.clone()
    }
}

pub fn playback_buffer_from_tts(audio: &AudioBuffer) -> Result<PlaybackBuffer, AudioOutputError> {
    if audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ {
        return Err(AudioOutputError::StreamConfig(format!(
            "unsupported sample rate: {}",
            audio.sample_rate_hz
        )));
    }
    if audio.channels != PLAYBACK_CHANNELS {
        return Err(AudioOutputError::StreamConfig(format!(
            "unsupported channel count: {}",
            audio.channels
        )));
    }

    let samples = match &audio.pcm {
        PcmData::F32(data) => data.clone(),
        PcmData::I16(data) => data.iter().map(|s| *s as f32 / 32768.0).collect(),
    };

    Ok(PlaybackBuffer::from_samples(samples))
}

pub struct AudioOutput {
    _stream: cpal::Stream,
    queue: Arc<Mutex<VecDeque<f32>>>,
}

impl AudioOutput {
    pub fn new() -> Result<Self, AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::NoOutputDevice)?;

        let mut supported = device
            .supported_output_configs()
            .map_err(|e| AudioOutputError::StreamConfig(e.to_string()))?;

        let supported = supported
            .find(|c| {
                c.channels() == PLAYBACK_CHANNELS
                    && c.min_sample_rate() <= PLAYBACK_SAMPLE_RATE_HZ
                    && c.max_sample_rate() >= PLAYBACK_SAMPLE_RATE_HZ
                    && c.sample_format() == cpal::SampleFormat::F32
            })
            .ok_or_else(|| {
                AudioOutputError::StreamConfig(
                    "No supported output config: need 48kHz stereo f32".to_string(),
                )
            })?;

        let stream_config = supported.with_sample_rate(PLAYBACK_SAMPLE_RATE_HZ).config();

        let queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let queue_for_cb = queue.clone();

        let stream = device
            .build_output_stream(
                &stream_config,
                move |output: &mut [f32], _| {
                    let mut queue = queue_for_cb.lock();
                    for sample in output.iter_mut() {
                        *sample = queue.pop_front().unwrap_or(0.0);
                    }
                },
                move |_| {},
                None,
            )
            .map_err(|e| AudioOutputError::StreamBuild(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioOutputError::StreamPlay(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            queue,
        })
    }

    pub fn enqueue(&self, buffer: PlaybackBuffer) {
        let mut queue = self.queue.lock();
        queue.extend(buffer.samples().iter().copied());
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    pub fn clear(&self) {
        self.queue.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tts_core::{AudioBuffer, PcmData};

    #[test]
    fn converts_i16_to_f32_for_playback() {
        let audio = AudioBuffer {
            sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
            channels: PLAYBACK_CHANNELS,
            pcm: PcmData::I16(vec![0, 16384, i16::MIN, i16::MAX]),
        };

        let buffer = playback_buffer_from_tts(&audio).expect("must convert");
        let samples = buffer.samples();
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], 0.5);
        assert_eq!(samples[2], -1.0);
        assert!((samples[3] - 32767.0 / 32768.0).abs() < f32::EPSILON);
    }
}
