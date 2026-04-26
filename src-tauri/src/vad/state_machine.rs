use crossbeam_channel::Sender;
use serde::Serialize;

use super::config::{MAX_RECORDING_SAMPLES, MIN_RECORDING_SAMPLES, SILENCE_FRAMES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VadState {
    Idle,
    Listening,
    Recording,
    Processing,
}

impl fmt::Display for VadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VadState::Idle => write!(f, "idle"),
            VadState::Listening => write!(f, "listening"),
            VadState::Recording => write!(f, "recording"),
            VadState::Processing => write!(f, "processing"),
        }
    }
}

use std::fmt;

#[derive(Debug, Clone)]
pub enum VadEvent {
    StateChanged(VadState),
    SpeechDetected(Vec<i16>),
    Error(String),
}

pub struct VadStateMachine {
    state: VadState,
    buffer: Vec<i16>,
    silence_counter: u32,
    event_tx: Sender<VadEvent>,
}

#[cfg(test)]
mod tests {
    use super::{VadEvent, VadState, VadStateMachine};
    use crossbeam_channel::unbounded;

    #[test]
    fn stop_resets_state_from_recording() {
        let (tx, _rx) = unbounded();
        let mut sm = VadStateMachine::new(tx);

        sm.start();
        sm.process_frame(&[1; 256], true);
        assert_eq!(sm.get_state(), VadState::Recording);

        sm.stop();
        assert_eq!(sm.get_state(), VadState::Idle);
    }

    #[test]
    fn finish_transcription_returns_to_idle() {
        let (tx, rx) = unbounded();
        let mut sm = VadStateMachine::new(tx);

        sm.start();
        sm.process_frame(&[1; 256], true);
        sm.finish_transcription();

        assert_eq!(sm.get_state(), VadState::Idle);
        let events: Vec<VadEvent> = rx.try_iter().collect();
        assert!(events
            .iter()
            .any(|event| matches!(event, VadEvent::StateChanged(VadState::Idle))));
    }
}

impl VadStateMachine {
    pub fn new(event_tx: Sender<VadEvent>) -> Self {
        Self {
            state: VadState::Idle,
            buffer: Vec::with_capacity(MAX_RECORDING_SAMPLES),
            silence_counter: 0,
            event_tx,
        }
    }

    pub fn start(&mut self) {
        self.state = VadState::Listening;
        self.buffer.clear();
        self.silence_counter = 0;
        let _ = self
            .event_tx
            .send(VadEvent::StateChanged(VadState::Listening));
    }

    pub fn stop(&mut self) {
        self.state = VadState::Idle;
        self.buffer.clear();
        self.silence_counter = 0;
        let _ = self.event_tx.send(VadEvent::StateChanged(VadState::Idle));
    }

    pub fn process_frame(&mut self, audio: &[i16], is_speech: bool) {
        match self.state {
            VadState::Idle | VadState::Processing => {}
            VadState::Listening => {
                if is_speech {
                    self.state = VadState::Recording;
                    self.buffer.clear();
                    self.buffer.extend_from_slice(audio);
                    self.silence_counter = 0;
                    let _ = self
                        .event_tx
                        .send(VadEvent::StateChanged(VadState::Recording));
                }
            }
            VadState::Recording => {
                self.buffer.extend_from_slice(audio);

                if self.buffer.len() > MAX_RECORDING_SAMPLES {
                    self.buffer.truncate(MAX_RECORDING_SAMPLES);
                }

                if is_speech {
                    self.silence_counter = 0;
                } else {
                    self.silence_counter += 1;
                    if self.silence_counter >= SILENCE_FRAMES {
                        let audio_data = std::mem::take(&mut self.buffer);
                        self.silence_counter = 0;
                        self.state = VadState::Processing;
                        let _ = self
                            .event_tx
                            .send(VadEvent::StateChanged(VadState::Processing));

                        if audio_data.len() >= MIN_RECORDING_SAMPLES {
                            let _ = self.event_tx.send(VadEvent::SpeechDetected(audio_data));
                        } else {
                            self.state = VadState::Idle;
                            let _ = self.event_tx.send(VadEvent::StateChanged(VadState::Idle));
                        }
                    }
                }
            }
        }
    }

    pub fn finish_transcription(&mut self) {
        self.state = VadState::Idle;
        self.buffer.clear();
        self.silence_counter = 0;
        let _ = self.event_tx.send(VadEvent::StateChanged(VadState::Idle));
    }

    pub fn get_state(&self) -> VadState {
        self.state
    }
}
