# AGENTS.md

Model session and embedding loading for Qwen3 ASR.

## Module Map
- `session.rs` locates model assets, creates ONNX Runtime sessions, loads embeddings, and defines runtime wrappers.
- `session_manager_adapter.rs` adapts loaded ONNX sessions to the `stt_core::SessionManager` trait for encoder/decoder code and tests.
- `mod.rs` organizes the model submodules.

## Relationships
- Used by `encoder.rs` and `decoder.rs` through session manager APIs.
- Model file expectations should stay aligned with `docs/model_inputs_spec.json` and `scripts/verify_onnx_inputs.py`.

## Editing Notes
- Keep asset errors precise; model setup failures are surfaced to users through ASR status.
- Avoid repeated session creation during transcription; model load should happen once in `Qwen3AsrEngine::new`.
