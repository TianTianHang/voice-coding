# AGENTS.md

Audio input/output primitives used by VAD recording and TTS playback.

## Module Map
- `mod.rs` re-exports recorder and playback helpers.
- `recorder.rs` captures microphone PCM audio, feeds frames through the VAD engine, buffers utterance audio, and sends recorder events to `vad_commands.rs`.
- `output.rs` converts `tts_core::AudioBuffer` into playback buffers and manages audio output playback/cancelation.

## Relationships
- `vad_commands.rs` owns recorder lifecycle and session ids; this folder should not emit frontend events directly unless the command layer owns the payload contract.
- `recorder.rs` uses `src-tauri/src/vad/` for TEN VAD decisions.
- `output.rs` is used by `src-tauri/src/tts.rs` and must respect `tts_core` playback constraints: 48 kHz stereo.

## Editing Notes
- Keep sample-rate/channel assumptions explicit at module boundaries.
- Avoid holding locks while invoking callbacks or doing blocking audio operations.
- Cleanly stop streams and threads on drop/stop to prevent stuck devices.
- Convert platform/device errors into typed errors internally, then strings at Tauri commands.
