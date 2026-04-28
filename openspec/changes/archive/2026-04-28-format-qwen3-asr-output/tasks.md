## 1. Parser

- [x] 1.1 Add a Qwen3 ASR output parser that accepts decoded raw output and an optional forced language.
- [x] 1.2 Support `<asr_text>` metadata splitting, newline-separated metadata, no-tag fallback, and `language None<asr_text>` empty-audio handling.
- [x] 1.3 Add conservative language normalization for known Qwen3 language labels to existing supported language codes.

## 2. Engine Integration

- [x] 2.1 Wire the parser into `Qwen3AsrEngine::transcribe_samples` after tokenizer decode and before constructing `SttResult`.
- [x] 2.2 Preserve forced-language output semantics by returning the configured language and treating decoded output as text-only.
- [x] 2.3 Preserve auto-mode fallback by returning `"auto"` when no language metadata is present.
- [x] 2.4 Ensure chunked transcription continues to append cleaned chunk text without leaking metadata.

## 3. Tests

- [x] 3.1 Add parser unit tests for tagged auto output, newline metadata, no-tag output, forced-language output, empty audio, and `language None` with returned text.
- [x] 3.2 Add or update engine-level tests where practical to prove `SttResult.text` and `SttResult.language` use parsed values.

## 4. Verification

- [x] 4.1 Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] 4.2 Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`.
- [x] 4.3 Record any unavailable model-dependent checks or environmental blockers in the implementation summary.
