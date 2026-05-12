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

    pub fn duration(&self) -> std::time::Duration {
        duration_from_sample_count(self.samples.len())
    }
}

pub fn duration_from_sample_count(samples: usize) -> std::time::Duration {
    let frames = samples / PLAYBACK_CHANNELS as usize;
    std::time::Duration::from_secs_f64(frames as f64 / PLAYBACK_SAMPLE_RATE_HZ as f64)
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
    engine: Arc<Mutex<PlaybackSampleEngine>>,
}

impl AudioOutput {
    pub fn new() -> Result<Self, AudioOutputError> {
        log::info!("creating audio output stream");
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::NoOutputDevice)?;
        log::debug!(
            "selected output device: {}",
            device
                .description()
                .map(|description| description.name().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        );

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
        log::info!(
            "output stream config selected: channels={} sample_rate={} buffer_size={:?}",
            stream_config.channels,
            stream_config.sample_rate,
            stream_config.buffer_size
        );

        let engine = Arc::new(Mutex::new(PlaybackSampleEngine::new()));
        let engine_for_cb = engine.clone();

        let stream = device
            .build_output_stream(
                &stream_config,
                move |output: &mut [f32], _| {
                    engine_for_cb.lock().fill_output(output);
                },
                move |err| {
                    log::error!("output stream error: {err}");
                },
                None,
            )
            .map_err(|e| AudioOutputError::StreamBuild(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioOutputError::StreamPlay(e.to_string()))?;

        log::info!("audio output stream started");
        Ok(Self {
            _stream: stream,
            engine,
        })
    }

    pub fn enqueue(&self, buffer: PlaybackBuffer) {
        log::debug!(
            "enqueueing playback buffer: samples={} duration_ms={}",
            buffer.samples().len(),
            buffer.duration().as_millis()
        );
        self.engine.lock().enqueue(buffer);
    }

    pub fn queued_duration(&self) -> std::time::Duration {
        self.engine.lock().queued_duration()
    }

    pub fn configure_adaptive_playback(&self, target_queue_sec: f64) {
        self.engine
            .lock()
            .configure_adaptive_playback(target_queue_sec);
    }

    pub fn clear(&self) {
        log::debug!("clearing audio output queue");
        self.engine.lock().clear();
    }
}

const PLAYBACK_CHANNELS_USIZE: usize = PLAYBACK_CHANNELS as usize;
const PLC_HISTORY_MS: usize = 20;
const PLC_MAX_MS: usize = 120;
const PLC_HISTORY_SAMPLES: usize =
    PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS_USIZE * PLC_HISTORY_MS / 1_000;
const PLC_MAX_SAMPLES: usize =
    PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS_USIZE * PLC_MAX_MS / 1_000;
const PLAYBACK_RATIO_MIN: f64 = 0.985;
const PLAYBACK_RATIO_MAX: f64 = 1.020;

#[derive(Debug)]
struct PlaybackSampleEngine {
    queue: VecDeque<f32>,
    ratio: f64,
    ratio_target: f64,
    read_phase: f64,
    plc_history: VecDeque<f32>,
    plc_cursor_frames: usize,
    plc_samples_emitted: usize,
}

impl PlaybackSampleEngine {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            ratio: 1.0,
            ratio_target: 1.0,
            read_phase: 0.0,
            plc_history: VecDeque::with_capacity(PLC_HISTORY_SAMPLES),
            plc_cursor_frames: 0,
            plc_samples_emitted: 0,
        }
    }

    fn enqueue(&mut self, buffer: PlaybackBuffer) {
        self.queue.extend(buffer.samples().iter().copied());
        if !self.queue.is_empty() {
            self.plc_samples_emitted = 0;
        }
    }

    fn queued_duration(&self) -> std::time::Duration {
        duration_from_sample_count(self.queue.len())
    }

    fn configure_adaptive_playback(&mut self, target_queue_sec: f64) {
        let queued_sec = self.queued_duration().as_secs_f64();
        let diff = queued_sec - target_queue_sec.max(0.0);
        let desired = if diff > 0.45 {
            PLAYBACK_RATIO_MAX
        } else if diff > 0.20 {
            1.010
        } else if diff < -0.30 && queued_sec > 0.05 {
            PLAYBACK_RATIO_MIN
        } else if diff < -0.12 && queued_sec > 0.05 {
            0.993
        } else {
            1.0
        };
        self.ratio_target = desired;
    }

    fn clear(&mut self) {
        self.queue.clear();
        self.ratio = 1.0;
        self.ratio_target = 1.0;
        self.read_phase = 0.0;
        self.plc_history.clear();
        self.plc_cursor_frames = 0;
        self.plc_samples_emitted = 0;
    }

    fn fill_output(&mut self, output: &mut [f32]) {
        for frame in output.chunks_mut(PLAYBACK_CHANNELS_USIZE) {
            if let Some(samples) = self.next_audio_frame() {
                frame.copy_from_slice(&samples);
                self.remember_frame(&samples);
                self.plc_samples_emitted = 0;
            } else {
                let samples = self.next_plc_frame();
                frame.copy_from_slice(&samples);
            }
        }
    }

    fn next_audio_frame(&mut self) -> Option<[f32; PLAYBACK_CHANNELS_USIZE]> {
        self.ratio += (self.ratio_target - self.ratio) * 0.02;
        self.consume_whole_frames(self.read_phase.floor() as usize);
        self.read_phase = self.read_phase.fract();

        if self.queue.len() < PLAYBACK_CHANNELS_USIZE {
            self.read_phase = 0.0;
            return None;
        }

        let frac = self.read_phase as f32;
        let mut frame = [0.0; PLAYBACK_CHANNELS_USIZE];
        for (channel, sample) in frame.iter_mut().enumerate() {
            let current = self.queue.get(channel).copied().unwrap_or(0.0);
            let next = self
                .queue
                .get(PLAYBACK_CHANNELS_USIZE + channel)
                .copied()
                .unwrap_or(current);
            *sample = current + (next - current) * frac;
        }

        self.read_phase += self.ratio;
        Some(frame)
    }

    fn consume_whole_frames(&mut self, frames: usize) {
        let samples = frames
            .saturating_mul(PLAYBACK_CHANNELS_USIZE)
            .min(self.queue.len() / PLAYBACK_CHANNELS_USIZE * PLAYBACK_CHANNELS_USIZE);
        for _ in 0..samples {
            self.queue.pop_front();
        }
    }

    fn remember_frame(&mut self, frame: &[f32; PLAYBACK_CHANNELS_USIZE]) {
        for sample in frame {
            if self.plc_history.len() == PLC_HISTORY_SAMPLES {
                self.plc_history.pop_front();
            }
            self.plc_history.push_back(*sample);
        }
    }

    fn next_plc_frame(&mut self) -> [f32; PLAYBACK_CHANNELS_USIZE] {
        if self.plc_history.len() < PLAYBACK_CHANNELS_USIZE
            || self.plc_samples_emitted >= PLC_MAX_SAMPLES
        {
            return [0.0; PLAYBACK_CHANNELS_USIZE];
        }

        let history_frames = self.plc_history.len() / PLAYBACK_CHANNELS_USIZE;
        let source_frame = self.plc_cursor_frames % history_frames;
        let fade = 1.0 - (self.plc_samples_emitted as f32 / PLC_MAX_SAMPLES as f32);
        let mut frame = [0.0; PLAYBACK_CHANNELS_USIZE];
        for (channel, sample) in frame.iter_mut().enumerate() {
            let index = source_frame * PLAYBACK_CHANNELS_USIZE + channel;
            *sample = self.plc_history.get(index).copied().unwrap_or(0.0) * fade;
        }
        self.plc_cursor_frames = self.plc_cursor_frames.wrapping_add(1);
        self.plc_samples_emitted += PLAYBACK_CHANNELS_USIZE;
        frame
    }

    #[cfg(test)]
    fn queued_samples(&self) -> usize {
        self.queue.len()
    }

    #[cfg(test)]
    fn set_ratio_target_for_test(&mut self, ratio: f64) {
        self.ratio = ratio;
        self.ratio_target = ratio;
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

    #[test]
    fn playback_buffer_reports_duration_from_frames() {
        let buffer = PlaybackBuffer::from_samples(vec![0.0; PLAYBACK_SAMPLE_RATE_HZ as usize * 2]);

        assert_eq!(buffer.duration(), std::time::Duration::from_secs(1));
    }

    #[test]
    fn duration_from_sample_count_reports_stereo_queue_duration() {
        assert_eq!(
            duration_from_sample_count(
                PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS as usize / 2
            ),
            std::time::Duration::from_millis(500)
        );
    }

    #[test]
    fn playback_engine_uses_plc_when_queue_runs_short() {
        let mut engine = PlaybackSampleEngine::new();
        engine.enqueue(PlaybackBuffer::from_samples(vec![
            0.25;
            PLAYBACK_CHANNELS as usize
                * 8
        ]));
        let mut output = vec![0.0; PLAYBACK_CHANNELS as usize * 8];
        engine.fill_output(&mut output);

        let mut concealed = vec![0.0; PLAYBACK_CHANNELS as usize * 4];
        engine.fill_output(&mut concealed);

        assert!(concealed.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn playback_engine_fades_to_silence_after_plc_limit() {
        let mut engine = PlaybackSampleEngine::new();
        engine.enqueue(PlaybackBuffer::from_samples(vec![
            0.5;
            PLAYBACK_CHANNELS as usize
                * 16
        ]));
        let mut output = vec![0.0; PLAYBACK_CHANNELS as usize * 16];
        engine.fill_output(&mut output);

        let mut concealed = vec![1.0; PLC_MAX_SAMPLES + PLAYBACK_CHANNELS as usize * 8];
        engine.fill_output(&mut concealed);

        let tail = &concealed[concealed.len() - PLAYBACK_CHANNELS as usize * 8..];
        assert!(tail.iter().all(|sample| sample.abs() <= f32::EPSILON));
    }

    #[test]
    fn playback_engine_ratio_above_one_consumes_queue_faster() {
        let samples = vec![0.1; PLAYBACK_CHANNELS as usize * 200];
        let mut normal = PlaybackSampleEngine::new();
        normal.enqueue(PlaybackBuffer::from_samples(samples.clone()));
        let mut faster = PlaybackSampleEngine::new();
        faster.enqueue(PlaybackBuffer::from_samples(samples));
        faster.set_ratio_target_for_test(1.020);

        let mut output = vec![0.0; PLAYBACK_CHANNELS as usize * 100];
        normal.fill_output(&mut output);
        faster.fill_output(&mut output);

        assert!(faster.queued_samples() < normal.queued_samples());
    }

    #[test]
    fn playback_engine_ratio_below_one_consumes_queue_slower() {
        let samples = vec![0.1; PLAYBACK_CHANNELS as usize * 200];
        let mut normal = PlaybackSampleEngine::new();
        normal.enqueue(PlaybackBuffer::from_samples(samples.clone()));
        let mut slower = PlaybackSampleEngine::new();
        slower.enqueue(PlaybackBuffer::from_samples(samples));
        slower.set_ratio_target_for_test(0.985);

        let mut output = vec![0.0; PLAYBACK_CHANNELS as usize * 100];
        normal.fill_output(&mut output);
        slower.fill_output(&mut output);

        assert!(slower.queued_samples() > normal.queued_samples());
    }

    #[test]
    fn playback_engine_clear_resets_queue_and_plc_state() {
        let mut engine = PlaybackSampleEngine::new();
        engine.enqueue(PlaybackBuffer::from_samples(vec![
            0.5;
            PLAYBACK_CHANNELS as usize
                * 8
        ]));
        let mut output = vec![0.0; PLAYBACK_CHANNELS as usize * 8];
        engine.fill_output(&mut output);
        engine.clear();

        let mut after_clear = vec![1.0; PLAYBACK_CHANNELS as usize * 4];
        engine.fill_output(&mut after_clear);

        assert_eq!(engine.queued_samples(), 0);
        assert!(after_clear
            .iter()
            .all(|sample| sample.abs() <= f32::EPSILON));
    }
}
