# AGENTS.md

Tokenizer wrapper for Qwen3 ASR prompt building and output decoding.

## Module Role
- Load tokenizer assets from the model directory.
- Encode prompt/control text when needed by `prompt.rs`.
- Decode generated token ids for `output.rs` parsing.

## Editing Notes
- Keep tokenizer errors mapped to `SttError::TokenizerError`.
- Changes here can affect language tags and transcript formatting; run prompt/output related tests.
