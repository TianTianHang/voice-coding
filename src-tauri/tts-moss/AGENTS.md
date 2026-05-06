# AGENTS.md

MOSS ONNX Text-to-Speech implementation crate. It validates model assets, tokenizes text, runs TTS/codec ONNX models, and returns playback-ready PCM audio through `tts-core`.

## Module Role
- `src/lib.rs` contains asset discovery, manifest/meta validation, voice selection, ONNX session setup, synthesis, and TTS trait implementation.
- `Cargo.toml` contains engine-specific dependencies such as ONNX Runtime, ndarray, and sentencepiece.

## Model Contract
- Tauri runtime resolves the model package through unified app-level model path management and passes the direct `MOSS-TTS-Nano-100M-ONNX` component directory to this crate.
- `MossModelConfig::from_env()` remains a crate-level compatibility entry; `MOSS_TTS_MODEL_DIR` still means the direct `MOSS-TTS-Nano-100M-ONNX` component directory.
- The standard app layout is `models/tts/moss-tts-nano-100m-onnx/`, with legacy `models/moss-tts/` supported by the Tauri runtime.
- Required assets are declared by `browser_poc_manifest.json`, TTS metadata, codec metadata, tokenizer model, and related ONNX/external-data files.
- Output must validate against `tts_core::TtsResult::validate_for_playback`: 48 kHz stereo PCM.

## Relationships
- Used by `src-tauri/src/tts.rs` behind the `tts-moss-onnx` feature.
- Models can be downloaded with `scripts/download_moss_tts_models.sh`.
- Frontend TTS controls live in `src/components/AssistantConsole.tsx`.

## Validation
- Run `nix develop -c cargo test -p tts-moss` for crate changes.
- Run `nix develop -c cargo test -p voice-coding` when changing runtime integration behavior.
