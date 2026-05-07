pub mod config;
pub mod engine;
pub mod state_machine;

pub use config::{SAMPLE_RATE, THRESHOLD, VadConfig};
pub use engine::{VadEngine, VadError};
pub use state_machine::{VadEvent, VadState, VadStateMachine};
