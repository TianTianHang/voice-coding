## 1. Official Behavior Inventory

- [x] 1.1 Review official `text_normalization_pipeline.py` and `tts_robust_normalizer_single_script.py` and record the robust rule categories to port.
- [x] 1.2 Decide which rules are generic MOSS normalization and which are Agent auto-TTS speakable-text policy.
- [x] 1.3 Create representative fixtures from real Agent replies, Markdown, code snippets, paths, URLs, version numbers, mixed Chinese/English, and repeated punctuation.

## 2. MOSS Robust Normalizer

- [x] 2.1 Add a Rust robust normalization module under `src-tauri/tts-moss/src/`.
- [x] 2.2 Implement zero-width/control character removal, whitespace/newline normalization, punctuation collapsing, arrow/dash handling, and full-width punctuation handling.
- [x] 2.3 Implement Markdown syntax cleanup for headings, block quotes, lists, emphasis, tables, links, fenced code delimiters, and inline code delimiters.
- [x] 2.4 Implement protected-span handling for URLs, email addresses, mentions, hashtags, file names, dot-version strings, and short technical identifiers.
- [x] 2.5 Integrate robust normalization before token-budget chunking in `MossTextPreprocessor`.
- [x] 2.6 Ensure empty or symbol-only inputs become empty chunks and fail with the existing speakable-text error instead of synthesizing unstable audio.

## 3. Agent Speakable Text

- [x] 3.1 Add an auto-TTS text preparation helper for Agent `result` content before `speak_agent_result`.
- [x] 3.2 Drop or summarize fenced code blocks, diffs, long JSON, terminal logs, and command-heavy sections for automatic speech.
- [x] 3.3 Preserve the raw Agent event content for UI display and duplicate detection where appropriate.
- [x] 3.4 Ensure replay uses the prepared speakable text consistently or stores enough state to regenerate it deterministically.

## 4. Tests

- [x] 4.1 Add `tts-moss` unit tests for robust normalizer fixtures covering official behavior categories.
- [x] 4.2 Add tests for symbol-heavy Agent replies that previously caused skipped reading or strange sounds.
- [x] 4.3 Add tests for protected spans such as `.env`, `app.js.map`, `v2.3.1`, URLs, email addresses, and `foo_bar`.
- [x] 4.4 Add backend auto-TTS tests proving `result.content` is prepared for speech without changing emitted Agent events.
- [x] 4.5 Run `nix develop -c cargo test -p tts-moss` and relevant backend tests.

## 5. Manual Validation

- [x] 5.1 Use debug TTS with copied real Agent replies before and after normalization.
- [x] 5.2 Validate end-to-end auto speech for replies containing lists, inline code, paths, arrows, and repeated punctuation.
- [x] 5.3 Document any remaining known gaps that require WeText or later semantic normalization.
