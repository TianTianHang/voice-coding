## Context

The current `MossTextPreprocessor` performs only lightweight normalization:

```
raw text
  -> trim/collapse whitespace
  -> normalize a small set of full-width punctuation
  -> insert CJK/ASCII spacing
  -> append terminal punctuation
  -> chunk by token budget
  -> tokenize and synthesize
```

This is enough for simple debug strings such as `你好。`, but automatic Agent replies frequently look like:

```
已完成：
- 修改 `src-tauri/tts-moss/src/text.rs`
- 运行 `nix develop -c cargo test -p tts-moss`

注意 `.env`、v2.3.1、foo_bar() 和 a -> b 这些符号。
```

Those are meaningful on screen but hostile to speech synthesis. The official MOSS-TTS-Nano text pipeline uses `text_normalization_pipeline.py` to orchestrate language-aware normalization and `tts_robust_normalizer_single_script.py` to clean robustly around Markdown, URLs, file names, underscores, arrows, repeated punctuation, and invisible/control characters.

## Goals

- Move our Rust normalizer closer to official MOSS robust normalization for symbol-heavy real-world text.
- Make backend auto TTS synthesize speakable content instead of raw display Markdown.
- Keep the implementation deterministic, offline, and testable without Python or network dependencies.
- Preserve safe technical context where it is useful to hear, while avoiding dense code or markup that destabilizes synthesis.

## Non-Goals

- Full WeText semantic normalization is not included in the first implementation. We can revisit it after robust normalization is stable.
- The UI should still render raw Agent text with Markdown-like structure where it already does.
- The TTS engine should not silently read large code blocks, diffs, or command output verbatim.

## Decisions

### 1) Port the robust normalizer into Rust, not Python

The implementation should translate the behavior of the official robust normalizer into a Rust module. This avoids adding Python process management and keeps Tauri commands deterministic. The port does not need to preserve line-for-line structure, but it should preserve the behavior classes we depend on.

Expected rule groups:

- remove zero-width and control characters;
- collapse whitespace and normalize newlines into sentence boundaries;
- strip Markdown headings, block quotes, list markers, emphasis delimiters, and table separators;
- remove fenced code block delimiters and either summarize or drop the code body depending on call-site policy;
- remove inline-code delimiters while keeping short natural identifiers readable;
- simplify Markdown links by preserving link text and optionally readable URL/domain text;
- protect URL, email, mention, hashtag, file-name, and version-like spans before generic symbol replacement;
- convert arrows and long dash runs into punctuation pauses;
- replace underscores and slash-heavy separators with spaces only when doing so will not corrupt protected spans;
- collapse repeated punctuation to stable sentence punctuation.

### 2) Separate generic MOSS normalization from Agent speakable-text policy

Two related layers should exist:

```
Agent result content
  -> agent speakable text policy
  -> official-style robust MOSS normalizer
  -> token-budget chunking
  -> MOSS inference
```

The generic MOSS normalizer belongs in `tts-moss` and runs for every synthesis request, including debug TTS. The Agent speakable-text policy belongs in the backend auto-TTS path and can decide to drop or summarize content that should not be spoken verbatim.

This separation keeps manual debug synthesis useful for testing arbitrary text, while auto TTS can be more selective for real assistant replies.

### 3) First implementation should prefer deletion or short labels over verbose symbol names

For voice-driven coding, hearing every symbol name is noisy. For example, reading backticks as "反引号" or underscores as "下划线" makes responses exhausting. The first policy should:

- drop Markdown syntax characters;
- turn line/list boundaries into pauses;
- keep concise natural-language descriptions;
- replace large code/diff/JSON blocks with short spoken labels only if needed;
- avoid reading long paths or commands verbatim unless they are the main answer.

### 4) Preserve enough debug visibility to diagnose normalization

Because this class of bugs is heard rather than seen, tests and optional debug status should expose normalized text. At minimum, unit tests must show raw input and normalized output. A future debug panel field could display normalized text, but that is not required for the first implementation.

## Risks / Trade-offs

- [Behavior drift from official Python] The Rust port may not match every edge case. Mitigate with fixtures covering the official rule categories we actually need.
- [Over-cleaning technical answers] Removing too much detail could make spoken replies less useful. Mitigate by keeping UI display unchanged and adding focused fixtures for common coding replies.
- [Language-specific ambiguity] Without WeText, some dates/numbers/units may remain imperfect. This change targets symbol robustness first.
- [Regex complexity] A large regex-only port could become fragile. Prefer small named functions and staged protected spans.

## Open Questions

- Should auto TTS skip code blocks entirely or say a short phrase such as "包含一段代码"?
- Should URLs/domains be spoken at all in automatic replies, or only link labels?
- Should we add a debug command to return normalized text without synthesizing audio?

