# AGENTS.md

Source modules for the Qwen3 ASR engine. Keep model-specific inference details here and expose a clean `Qwen3AsrEngine` from `lib.rs`.

## Data Flow
- Audio enters through `AudioInput` and is normalized/loaded by `audio::loader`.
- Samples are converted to mel features by `audio::mel`.
- `encoder` produces acoustic embeddings through ONNX.
- `prompt` and `tokenizer` prepare decoder inputs and decode output ids.
- `decoder` autoregressively generates tokens using sessions and embeddings.
- `output` strips Qwen formatting and resolves final language/text.

## Editing Notes
- Keep ONNX tensor shape assumptions close to the code that constructs tensors.
- Prefer testable pure helpers for prompt, output parsing, VAD splitting, and boundary behavior.
- Avoid adding Tauri-specific types here; communicate through `stt-core`.
