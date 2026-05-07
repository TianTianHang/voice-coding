use std::fmt;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SizedSample};
use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;

use crate::vad::{VadConfig, VadEngine, VadEvent, VadStateMachine};

#[derive(Debug)]
pub enum RecorderError {
    NoInputDevice,
    StreamConfig(String),
    StreamBuild(String),
    StreamPlay(String),
    VadEngine(crate::vad::VadError),
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::NoInputDevice => write!(f, "No input device found"),
            RecorderError::StreamConfig(s) => write!(f, "Stream config error: {}", s),
            RecorderError::StreamBuild(s) => write!(f, "Stream build error: {}", s),
            RecorderError::StreamPlay(s) => write!(f, "Stream play error: {}", s),
            RecorderError::VadEngine(e) => write!(f, "VAD engine error: {}", e),
        }
    }
}

impl std::error::Error for RecorderError {}

impl From<crate::vad::VadError> for RecorderError {
    fn from(e: crate::vad::VadError) -> Self {
        RecorderError::VadEngine(e)
    }
}

pub struct AudioRecorder {
    _stream: cpal::Stream,
    state_machine: Arc<Mutex<VadStateMachine>>,
    event_rx: Receiver<VadEvent>,
}

impl AudioRecorder {
    pub fn new(lib_path: &std::path::Path, config: &VadConfig) -> Result<Self, RecorderError> {
        log::info!(
            "creating audio recorder: vad_lib={} sample_rate={} hop_size={} threshold={}",
            lib_path.display(),
            config.sample_rate,
            config.hop_size,
            config.threshold
        );
        let vad_engine = VadEngine::new(lib_path, config.hop_size as i32, config.threshold)?;

        let (event_tx, event_rx): (Sender<VadEvent>, Receiver<VadEvent>) = unbounded();
        let error_tx = event_tx.clone();
        let state_machine = Arc::new(Mutex::new(VadStateMachine::new(event_tx)));
        let sm_clone = state_machine.clone();

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(RecorderError::NoInputDevice)?;
        log::debug!(
            "selected input device: {}",
            device
                .description()
                .map(|description| description.name().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        );

        let mut supported_configs = device
            .supported_input_configs()
            .map_err(|e| RecorderError::StreamConfig(e.to_string()))?;
        let supported_config = supported_configs
            .find(|c| c.channels() <= 2 && is_supported_sample_format(c.sample_format()))
            .ok_or_else(|| {
                RecorderError::StreamConfig(
                    "No supported input config: need mono/stereo integer or float samples".into(),
                )
            })?;
        let input_sample_rate = supported_sample_rate(&supported_config, config.sample_rate);
        let sample_format = supported_config.sample_format();
        let stream_config = supported_config
            .with_sample_rate(input_sample_rate)
            .config();

        let channels = stream_config.channels as usize;
        log::info!(
            "input stream config selected: channels={} sample_rate={} buffer_size={:?}",
            stream_config.channels,
            stream_config.sample_rate,
            stream_config.buffer_size
        );

        let processor = InputProcessor {
            vad_engine,
            state_machine: sm_clone,
            channels,
            input_sample_rate: stream_config.sample_rate,
            target_sample_rate: config.sample_rate,
            hop_size: config.hop_size,
            frame_buffer: Vec::with_capacity(config.hop_size * 4),
            resample_position: 0.0,
        };

        let stream = build_input_stream_for_format(
            &device,
            &stream_config,
            sample_format,
            processor,
            error_tx,
        )?;

        stream
            .play()
            .map_err(|e| RecorderError::StreamPlay(e.to_string()))?;

        log::info!("audio recorder started");
        Ok(Self {
            _stream: stream,
            state_machine,
            event_rx,
        })
    }

    pub fn state_machine(&self) -> Arc<Mutex<VadStateMachine>> {
        self.state_machine.clone()
    }

    pub fn event_rx(&self) -> Receiver<VadEvent> {
        self.event_rx.clone()
    }
}

struct InputProcessor {
    vad_engine: VadEngine,
    state_machine: Arc<Mutex<VadStateMachine>>,
    channels: usize,
    input_sample_rate: u32,
    target_sample_rate: u32,
    hop_size: usize,
    frame_buffer: Vec<i16>,
    resample_position: f64,
}

impl InputProcessor {
    fn process_samples<T>(&mut self, data: &[T])
    where
        T: Sample,
        f32: cpal::FromSample<T>,
    {
        let mono = downmix_input_to_mono(data, self.channels);
        let resampled = resample_mono_stream(
            &mono,
            self.input_sample_rate,
            self.target_sample_rate,
            &mut self.resample_position,
        );

        self.frame_buffer
            .extend(resampled.into_iter().map(f32_to_i16_sample));

        while self.frame_buffer.len() >= self.hop_size {
            let frame = &self.frame_buffer[..self.hop_size];
            if let Ok((_, flag)) = self.vad_engine.process(frame) {
                let is_speech = flag == 1;
                let mut sm = self.state_machine.lock();
                sm.process_frame(frame, is_speech);
            }
            self.frame_buffer.drain(..self.hop_size);
        }
    }
}

fn build_input_stream_for_format(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    processor: InputProcessor,
    error_tx: Sender<VadEvent>,
) -> Result<cpal::Stream, RecorderError> {
    match sample_format {
        cpal::SampleFormat::F32 => {
            build_typed_input_stream::<f32>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::F64 => {
            build_typed_input_stream::<f64>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::I8 => {
            build_typed_input_stream::<i8>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::I16 => {
            build_typed_input_stream::<i16>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::I32 => {
            build_typed_input_stream::<i32>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::U8 => {
            build_typed_input_stream::<u8>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::U16 => {
            build_typed_input_stream::<u16>(device, stream_config, processor, error_tx)
        }
        cpal::SampleFormat::U32 => {
            build_typed_input_stream::<u32>(device, stream_config, processor, error_tx)
        }
        other => Err(RecorderError::StreamConfig(format!(
            "Unsupported input sample format: {other}"
        ))),
    }
}

fn build_typed_input_stream<T>(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    mut processor: InputProcessor,
    error_tx: Sender<VadEvent>,
) -> Result<cpal::Stream, RecorderError>
where
    T: Sample + SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    device
        .build_input_stream::<T, _, _>(
            stream_config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                processor.process_samples(data);
            },
            move |err: cpal::StreamError| {
                log::error!("input stream error: {err}");
                let _ = error_tx.send(VadEvent::Error(err.to_string()));
            },
            None,
        )
        .map_err(|e| RecorderError::StreamBuild(e.to_string()))
}

fn is_supported_sample_format(format: cpal::SampleFormat) -> bool {
    matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::F64
            | cpal::SampleFormat::I8
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::I32
            | cpal::SampleFormat::U8
            | cpal::SampleFormat::U16
            | cpal::SampleFormat::U32
    )
}

fn supported_sample_rate(
    supported: &cpal::SupportedStreamConfigRange,
    preferred: u32,
) -> cpal::SampleRate {
    let min = supported.min_sample_rate();
    let max = supported.max_sample_rate();
    preferred.clamp(min, max)
}

fn downmix_input_to_mono<T>(data: &[T], channels: usize) -> Vec<f32>
where
    T: Sample,
    f32: cpal::FromSample<T>,
{
    if channels <= 1 {
        return data
            .iter()
            .map(|sample| sample.to_sample::<f32>())
            .collect();
    }

    data.chunks(channels)
        .map(|frame| {
            frame
                .iter()
                .map(|sample| sample.to_sample::<f32>())
                .sum::<f32>()
                / frame.len() as f32
        })
        .collect()
}

fn resample_mono_stream(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
    position: &mut f64,
) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    if samples.is_empty() {
        return Vec::new();
    }

    let step = from_rate as f64 / to_rate as f64;
    let mut output = Vec::with_capacity((samples.len() as f64 / step).ceil() as usize);
    while *position < samples.len() as f64 {
        let src_idx = *position as usize;
        let frac = *position - src_idx as f64;
        let sample = if src_idx + 1 < samples.len() {
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else {
            samples[src_idx]
        };
        output.push(sample);
        *position += step;
    }
    *position -= samples.len() as f64;
    output
}

fn f32_to_i16_sample(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_interleaved_stereo_to_mono() {
        let mono = downmix_input_to_mono(&[1.0_f32, 3.0, 2.0, 4.0], 2);

        assert_eq!(mono, vec![2.0, 3.0]);
    }

    #[test]
    fn resamples_stream_from_48k_to_16k() {
        let mut position = 0.0;
        let samples = vec![1.0_f32; 48_000];

        let resampled = resample_mono_stream(&samples, 48_000, 16_000, &mut position);

        assert_eq!(resampled.len(), 16_000);
        assert!(position.abs() < f64::EPSILON);
    }

    #[test]
    fn f32_to_i16_sample_clamps_to_valid_range() {
        assert_eq!(f32_to_i16_sample(2.0), i16::MAX);
        assert_eq!(f32_to_i16_sample(-2.0), -i16::MAX);
        assert_eq!(f32_to_i16_sample(0.0), 0);
    }
}
