## Why

Qwen3-ASR can emit transcription metadata such as `language Chinese<asr_text>` together with the user-visible transcript. The backend currently returns the decoded model string directly, which can leak metadata into the frontend transcript and leaves detected language information unused.

## What Changes

- Parse Qwen3-ASR decoded output before constructing `SttResult`.
- Strip ASR metadata from returned transcript text so Tauri commands and VAD events deliver clean text to the frontend.
- Populate `SttResult.language` from parsed model metadata when automatic language detection returns a language.
- Preserve the existing `"auto"` language value when no language metadata is present.
- Preserve forced-language behavior by treating model output as text-only and returning the configured language.
- Treat `language None<asr_text>` without transcript text as empty audio.

## Capabilities

### New Capabilities

### Modified Capabilities

- `stt-qwen3`: Qwen3-ASR results shall parse model output metadata into clean transcript text and language fields.

## Impact

- Affected Rust code: `src-tauri/stt-qwen3/src/lib.rs` and a small parser module or helper in the same crate.
- Affected API behavior: `SttResult.text` becomes clean transcript text instead of raw decoded model output; `SttResult.language` may contain a normalized detected language instead of always `"auto"` in auto mode.
- Frontend impact: existing transcript event handling receives cleaner text without additional frontend parsing.
- Dependencies: no new model, frontend, or runtime dependencies expected.
- Validation: add focused Rust unit tests for output parsing and run the Qwen3/STT Rust test suite.
