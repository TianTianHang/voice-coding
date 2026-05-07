## Why

Agent replies are written for screens, not speech. They often contain Markdown, inline code, paths, URLs, lists, arrows, version strings, and dense punctuation. The current MOSS text preparation only trims whitespace, normalizes a small punctuation set, inserts CJK/ASCII spacing, and chunks by token budget. In end-to-end auto TTS this leads to missed words, unstable pronunciation, and strange sounds when the model receives symbol-heavy text.

The official MOSS-TTS-Nano repository ships a text normalization pipeline with a robust pre/post normalizer that explicitly handles these noisy text forms. Porting the offline robust normalization behavior into Rust should make automatic agent speech substantially more reliable while keeping the desktop app self-contained.

## What Changes

- Add an official-inspired robust text normalization layer to `tts-moss` before tokenization and chunking.
- Normalize or remove speech-hostile markup and symbols including Markdown headings/lists/quotes, code fences, inline code delimiters, URLs, email addresses, mentions, hashtags, file/version-like dot sequences, arrows, underscores, repeated punctuation, zero-width characters, and control characters.
- Preserve text that should remain meaningful for speech, such as natural-language link labels, readable file names where appropriate, version numbers, and short identifiers.
- Apply the same robust normalization to manual debug TTS and backend-owned auto TTS because both enter `TtsEngine::synthesize`.
- Add an auto-TTS requirement that Agent `result` content must be converted into speakable text before synthesis while UI display remains unchanged.
- Add unit fixtures based on official-normalizer behaviors and real agent-reply examples.

## Non-Goals

- Do not vendor or execute the Python scripts at runtime.
- Do not add WeTextProcessing as a runtime dependency in this change.
- Do not attempt full semantic text normalization for every number/date/unit form beyond the robust normalizer rules.
- Do not change the visual Agent event stream content.
- Do not introduce streaming TTS playback or change the existing synthesize-then-play contract.

## Impact

- Rust backend: `src-tauri/tts-moss/src/text.rs` or a new normalization module under `src-tauri/tts-moss/src/`.
- Tests: `tts-moss` unit tests for normalizer fixtures, plus focused backend auto-TTS tests for speakable text selection.
- OpenSpec capabilities: `moss-onnx-tts-engine`, `backend-auto-tts`.

