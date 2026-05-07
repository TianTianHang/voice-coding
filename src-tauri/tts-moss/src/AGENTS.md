# AGENTS.md

Source for the MOSS ONNX TTS engine.

## Folder Role
- `lib.rs` is the crate entry point and keeps the unit tests plus split implementation includes.
- `config.rs` and `error.rs` define public configuration and crate errors.
- `assets.rs` loads model manifests, validates file layouts, and resolves built-in voices.
- `engine.rs` implements tokenizer setup, synthesis preparation, chunk concatenation, and the `TtsEngine` wrapper.
- `sessions.rs` owns ONNX session creation and TTS/codec inference helpers.
- `metadata.rs` contains manifest/meta structs and codec streaming state structures.
- `codec_buffer.rs`, `reference_audio.rs`, and `text.rs` contain focused codec PCM buffering, reference audio loading, and text preprocessing helpers.
- Keep all MOSS-specific assumptions in this crate and communicate with the app through `tts-core` types.

## Relationships
- Used by `src-tauri/src/tts.rs` when the `tts-moss-onnx` feature is enabled.
- Model files are downloaded by `scripts/download_moss_tts_models.sh` and found through `MOSS_TTS_MODEL_DIR` or the crate default.

## Editing Notes
- Output must remain playback-ready according to `tts_core::TtsResult::validate_for_playback`.
- Keep asset validation errors precise because they surface through frontend TTS status.
