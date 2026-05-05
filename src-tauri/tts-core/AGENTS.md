# AGENTS.md

Engine-agnostic Text-to-Speech contract crate. The Tauri TTS runtime and concrete engines share these types.

## Module Role
- Defines `TtsEngine`, optional streaming event types, and health check behavior.
- Defines `TtsConfig`, `TtsResult`, `AudioBuffer`, `PcmData`, and `TtsError`.
- Defines playback constraints: `PLAYBACK_SAMPLE_RATE_HZ` is 48 kHz and `PLAYBACK_CHANNELS` is stereo.

## Relationships
- `src-tauri/src/tts.rs` wraps a `dyn TtsEngine` and validates playback buffers.
- `src-tauri/tts-moss/` implements this trait for MOSS ONNX models.
- `src-tauri/src/audio/output.rs` consumes `AudioBuffer` for playback.

## Editing Notes
- Keep this crate free of Tauri and engine-specific dependencies.
- Changing playback constraints requires updating `audio/output.rs`, TTS engines, tests, and UI assumptions.
