# MOSS TTS Text Normalization

This document records the local Rust policy for preparing text before it is
sent to the MOSS ONNX TTS tokenizer. The goal is to stay close to the official
MOSS-TTS-Nano runtime while keeping the desktop app deterministic and free of a
Python runtime dependency.

## Pipeline

The effective path is:

```text
raw text
  -> language hint detection
  -> Chinese hyphen guard
  -> robust text cleanup
  -> character normalization and CJK/ASCII spacing
  -> sentence punctuation / short English handling
  -> token-budget chunking
  -> tokenizer
  -> MOSS ONNX inference
```

The implementation lives in:

- `src-tauri/tts-moss/src/text.rs` for language-aware preparation, sentence
  punctuation, short-English handling, and token-budget chunking.
- `src-tauri/tts-moss/src/robust.rs` for Markdown cleanup, protected technical
  spans, invisible/control character removal, and speech-hostile symbol cleanup.

## Character Rules

Character cleanup is applied before tokenization:

- Full-width ASCII letters and digits are mapped to half-width ASCII.
- Common full-width punctuation is normalized to stable punctuation.
- Curly single and double quotes are mapped to ASCII quotes.
- Zero-width characters and non-whitespace controls are removed.
- Newlines become sentence boundaries; tabs and repeated spaces collapse.
- Percent signs are verbalized as `percent`.
- `&` is verbalized as `and`.
- `+` and `=` are verbalized as `plus` / `equals` only when surrounded by
  ASCII alphanumeric text.

## Language-Sensitive Rules

The preprocessor treats text containing CJK characters as Chinese, text
containing ASCII letters as English, and otherwise falls back to Chinese.

For Chinese text:

- A missing terminal sentence mark is completed with `。`.
- A hyphen between Chinese words becomes a natural pause.
- A hyphen before a digit is preserved so negative numbers and numeric ranges
  are not turned into sentence boundaries.

For English text:

- The first alphabetic character is capitalized when it is lowercase.
- A terminal `.` is appended when the text ends with an alphanumeric character.
- Short English snippets keep the official runtime's leading-space convention.
  This leading space must survive chunk preparation and be included in the token
  IDs for the first chunk.

## Chunking Rules

The default MOSS text chunk budget is 75 tokenizer tokens. Text is first split
on sentence boundaries, then oversized sentences are split on softer boundaries
such as commas, colons, semicolons, and whitespace. Short adjacent pieces are
merged again when the merged text stays within the token budget.

Empty chunks are discarded. If normalization leaves no speakable content, the
TTS engine returns the existing invalid-input error instead of calling ONNX
inference.

## Known Gaps

This Rust implementation does not vendor or execute the official Python
`WeTextProcessing` pipeline. It therefore does not fully verbalize every date,
currency, long number, measurement, or language-specific abbreviation form. Add
new fixtures in `tts-moss` tests before changing these rules.
