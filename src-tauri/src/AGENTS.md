# AGENTS.md

Main Tauri application crate. This layer wires frontend commands/events to Rust runtime modules and handles desktop lifecycle behavior.

## Module Map
- `lib.rs` builds the Tauri app, registers managed state and commands, sets up tray behavior, and handles close/exit cleanup.
- `main.rs` is the binary entry point that calls the library runner.
- `asr.rs` owns ASR preload/status/transcription commands and the shared Qwen3 runtime.
- `tts.rs` owns TTS status/synthesis/playback commands and wraps the configured TTS engine.
- `vad_commands.rs` owns public VAD commands, runtime threshold config, session ids, and transcription handoff.
- `audio/` contains input recording and output playback primitives.
- `vad/` contains TEN VAD bindings/config/state-machine logic.
- `acp/` contains Agent Client Protocol transport, session management, profile resolution, and event mapping.

## Frontend Contract
- Command names in `generate_handler!` are part of the frontend API; update hooks/components together when renaming.
- Events expected by React include `vad-state`, `transcript`, `error`, `asr-status`, `tts-state`, and `agent-event`.
- Payload casing should use serde `camelCase` for TypeScript-friendly objects.

## Runtime Ownership
- Tauri managed state owns VAD recorder state, VAD runtime config, TTS runtime, ACP runtime, and app lifecycle state.
- Backend owns VAD session ids; increment on start and clear before/while stopping so late events can be dropped.
- Exit paths must stop listening and disconnect ACP before `app.exit(0)`.

## Editing Notes
- Prefer feature gates for optional engine-specific code.
- Convert internal errors with `.map_err(|e| e.to_string())?` at command boundaries.
- Use `parking_lot::Mutex` for short synchronous state and Tokio primitives for async shared state.
