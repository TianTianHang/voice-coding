pub mod config;
pub mod engine;
pub mod state_machine;

pub use config::{HOP_SIZE, SAMPLE_RATE, THRESHOLD};
pub use engine::{VadEngine, VadError};
pub use state_machine::{VadEvent, VadState, VadStateMachine};
