# AGENTS.md

Engine-agnostic Speech-to-Text contract crate. Implementations such as `stt-qwen3` depend on these types, and the Tauri ASR runtime depends only on this abstraction where possible.

## Module Role
- Defines `SttEngine`, `StreamingStt`, and `BatchStt` traits.
- Defines `AudioInput`, `SttConfig`, `SttResult`, `TimingInfo`, and `SttError`.
- Defines `SessionManager` and `KvCache` abstractions used by ONNX decoder/encoder layers.

## Relationships
- `src-tauri/src/asr.rs` calls engines through `SttEngine`.
- `src-tauri/src/vad_commands.rs` converts recorded PCM into `AudioInput::Samples`.
- `src-tauri/stt-qwen3/` implements these traits for Qwen3 ASR.

## Editing Notes
- Keep this crate free of Tauri, ONNX Runtime, and model-specific dependencies.
- API changes ripple into `stt-qwen3`, backend commands, and tests; update all together.
- Add shared errors here only when multiple engines can reasonably use them.
