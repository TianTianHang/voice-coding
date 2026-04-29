use std::fmt;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
        let vad_engine = VadEngine::new(lib_path, config.hop_size as i32, config.threshold)?;

        let (event_tx, event_rx): (Sender<VadEvent>, Receiver<VadEvent>) = unbounded();
        let error_tx = event_tx.clone();
        let state_machine = Arc::new(Mutex::new(VadStateMachine::new(event_tx)));
        let sm_clone = state_machine.clone();

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(RecorderError::NoInputDevice)?;

        let supported_config = device
            .supported_input_configs()
            .map_err(|e| RecorderError::StreamConfig(e.to_string()))?
            .find(|c| {
                c.channels() <= 2
                    && c.min_sample_rate() <= config.sample_rate
                    && c.max_sample_rate() >= config.sample_rate
                    && c.sample_format() == cpal::SampleFormat::I16
            })
            .ok_or_else(|| {
                RecorderError::StreamConfig("No supported input config: need 16kHz i16".into())
            })?;
        let stream_config = supported_config
            .with_sample_rate(config.sample_rate)
            .config();

        let channels = stream_config.channels as usize;

        let hop_size = config.hop_size;
        let mut frame_buffer: Vec<i16> = Vec::with_capacity(hop_size * 4);

        let stream = device
            .build_input_stream::<i16, _, _>(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mono_data: Vec<i16> = if channels == 1 {
                        data.to_vec()
                    } else {
                        data.iter().step_by(channels).copied().collect()
                    };

                    frame_buffer.extend_from_slice(&mono_data);

                    while frame_buffer.len() >= hop_size {
                        let frame = &frame_buffer[..hop_size];
                        if let Ok((_, flag)) = vad_engine.process(frame) {
                            let is_speech = flag == 1;
                            let mut sm = sm_clone.lock();
                            sm.process_frame(frame, is_speech);
                        }
                        frame_buffer.drain(..hop_size);
                    }
                },
                move |err: cpal::StreamError| {
                    let _ = error_tx.send(VadEvent::Error(err.to_string()));
                },
                None,
            )
            .map_err(|e| RecorderError::StreamBuild(e.to_string()))?;

        stream
            .play()
            .map_err(|e| RecorderError::StreamPlay(e.to_string()))?;

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
