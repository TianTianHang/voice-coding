# AGENTS.md

Voice Activity Detection engine wrapper and state machine. This folder detects speech boundaries; `vad_commands.rs` owns app sessions and frontend events.

## Module Map
- `config.rs` defines sample rate, threshold defaults, and `VadConfig`.
- `engine.rs` loads the native TEN VAD library and exposes frame-level speech probability/decision APIs.
- `state_machine.rs` converts VAD frame decisions into high-level states such as listening, recording, and processing.
- `mod.rs` re-exports the public VAD types used by audio and command modules.

## Relationships
- Native libraries live under `src-tauri/libs/<platform>/<arch>/` and are downloaded/built by scripts in `scripts/`.
- `audio/recorder.rs` feeds microphone frames into `VadEngine` and `VadStateMachine`.
- Frontend `VADState` in `src/hooks/useBackendVAD.ts` must stay compatible with serialized backend VAD states.

## Editing Notes
- Keep threshold, silence duration, and frame sizing changes coordinated with docs in `docs/TEN_VAD_*` and VAD tests.
- Use deterministic unit tests for state-machine transitions when changing boundary timing.
- Do not make this layer aware of Tauri windows, sessions, or ASR; keep it reusable.
