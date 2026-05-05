# AGENTS.md

Source for the MOSS ONNX TTS engine.

## Folder Role
- `lib.rs` handles model manifest loading, metadata validation, tokenizer setup, voice selection, ONNX inference, codec decoding, and `TtsEngine` implementation.
- Keep all MOSS-specific assumptions in this crate and communicate with the app through `tts-core` types.

## Relationships
- Used by `src-tauri/src/tts.rs` when the `tts-moss-onnx` feature is enabled.
- Model files are downloaded by `scripts/download_moss_tts_models.sh` and found through `MOSS_TTS_MODEL_DIR` or the crate default.

## Editing Notes
- Output must remain playback-ready according to `tts_core::TtsResult::validate_for_playback`.
- Keep asset validation errors precise because they surface through frontend TTS status.
