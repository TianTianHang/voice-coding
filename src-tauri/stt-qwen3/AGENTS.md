# AGENTS.md

Qwen3 ASR implementation crate. It loads ONNX model sessions and tokenizer assets, preprocesses audio, runs encoder/decoder inference, and returns `stt_core::SttResult`.

## Module Map
- `src/lib.rs` defines `Qwen3AsrEngine`, model loading timings, supported languages, and the `SttEngine` implementation.
- `src/audio/` loads/resamples audio, computes mel spectrograms, and splits long audio with VAD helpers.
- `src/models/` loads ONNX sessions and embeddings; adapters support testability around session managers.
- `src/encoder.rs` runs encoder inference.
- `src/decoder.rs` runs decoder initialization and autoregressive decoding.
- `src/prompt.rs` builds Qwen prompt token ids.
- `src/tokenizer/` wraps tokenizer loading/decoding.
- `src/output.rs` parses Qwen formatted output into final transcript/language.
- `tests/` contains unit/integration/boundary coverage and shared fixtures.

## Model Contract
- Default model root is provided by backend `STT_MODEL_DIR` handling, generally `./models`.
- Required assets include ONNX sessions under `onnx_models/`, plus embeddings and tokenizer files at the model root as expected by `models::session`.
- ONNX Runtime dynamic library path is supplied by the Nix environment via `ORT_DYLIB_PATH`.

## Relationships
- Implements `stt_core::SttEngine`; used by `src-tauri/src/asr.rs`.
- VAD-recorded audio reaches this crate as 16 kHz `AudioInput::Samples`.
- OpenSpec specs under `openspec/specs/stt-qwen3`, `onnx-inference`, and `audio-preprocessing` describe expected behavior.

## Validation
- Run `nix develop -c cargo test -p stt-qwen3` for crate changes.
- Use `nix develop -c cargo test -p stt-qwen3 --test integration_test` for model-backed integration behavior when assets are present.
- `run_tests.sh` and `TESTING.md` contain crate-specific test guidance.
