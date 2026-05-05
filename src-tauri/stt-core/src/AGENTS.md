# AGENTS.md

Source for the engine-agnostic STT contract crate.

## Folder Role
- `lib.rs` defines all public STT traits, errors, inputs, configs, timing, and result types.
- This folder should stay small and model-independent; concrete ASR engines live in sibling crates such as `src-tauri/stt-qwen3/`.

## Relationships
- `src-tauri/src/asr.rs` depends on these abstractions to avoid coupling command code to one engine.
- `src-tauri/stt-qwen3/src/` implements `SttEngine` and uses `SessionManager`/`KvCache` helpers from here.

## Editing Notes
- Do not add Tauri, ONNX Runtime, tokenizer, or model-asset dependencies here.
- Treat public type changes as cross-crate API changes and update all implementers/tests together.
