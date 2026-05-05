# AGENTS.md

Rust/Tauri workspace for the desktop backend. It owns OS integration, command registration, audio capture/playback, VAD sessions, ASR/TTS runtimes, and the ACP agent client.

## Workspace Map
- `src/` is the main `voice-coding` Tauri crate.
- `stt-core/` defines engine-agnostic STT traits and result types.
- `stt-qwen3/` implements Qwen3 ASR with ONNX sessions, tokenizer, audio preprocessing, and tests.
- `tts-core/` defines engine-agnostic TTS traits, PCM buffers, and playback constraints.
- `tts-moss/` implements MOSS ONNX TTS and model asset validation.
- `capabilities/` contains Tauri permission capability JSON.
- `icons/` contains packaged app icons.
- `libs/` contains native TEN VAD libraries copied or bundled for platforms.

## Cross-Crate Flow
- Frontend calls commands registered in `src/lib.rs`.
- `vad_commands.rs` creates `audio::AudioRecorder`, receives TEN VAD transitions, then calls `asr::get_stt_engine` for transcription.
- `asr.rs` wraps a shared `stt_core::SttEngine`, currently `stt_qwen3::Qwen3AsrEngine` behind the `stt-qwen3` feature.
- `tts.rs` wraps a shared `tts_core::TtsEngine`, currently `tts_moss::MossOnnxTtsEngine` behind `tts-moss-onnx`.
- `acp/` connects to external coding agents and emits frontend-friendly `agent-event` payloads.

## Commands And Features
- Run Cargo commands through Nix: `nix develop -c cargo test`, `nix develop -c cargo clippy`.
- Default features are `stt-qwen3` and `tts-moss-onnx`; use feature gates for optional engines.
- Add Rust dependencies with `cargo add`, not manual `Cargo.toml` edits.

## Editing Notes
- Tauri commands should return `Result<T, String>` and convert rich errors at the boundary.
- Keep long-running inference/model loading off the UI thread with async tasks or `spawn_blocking`.
- Session cleanup matters: stop VAD and disconnect ACP before app exit.
- Do not edit generated `target/` or `gen/schemas/` outputs by hand.
