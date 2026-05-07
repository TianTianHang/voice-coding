## Official Behavior Inventory

The official Python scripts are not vendored in this repository, so this Rust port uses the OpenSpec proposal, design, and requirement deltas as the local behavior inventory. The robust rule categories ported are:

- invisible/control character removal;
- full-width ASCII and punctuation normalization;
- whitespace and newline conversion into stable sentence boundaries;
- Markdown cleanup for headings, quotes, lists, emphasis, tables, links, fenced code markers, and inline code markers;
- protected-span staging for URLs, email-like text, mentions, hashtags, dot-files, file/version-like identifiers, and short underscore identifiers;
- arrow, dash-run, underscore, slash, bracket, and repeated punctuation stabilization;
- empty/symbol-only output detection before MOSS inference.

## Policy Split

Generic MOSS normalization lives in `tts-moss` and runs for every `TtsEngine::synthesize` request, including debug TTS. Agent result speakable-text policy lives in the backend auto-TTS path and runs before `speak_agent_result` synthesis. That policy drops or summarizes code fences, diff/log/JSON/command-heavy lines, while preserving the raw Agent event content for UI display, status snapshots, replay source text, and duplicate keys.

## Representative Fixtures

The unit fixtures cover real reply shapes such as Markdown lists with inline code paths, fenced code, command-heavy lines, mixed Chinese/English text, `.env`, `app.js.map`, `v2.3.1`, URLs, email/mention/hashtag-like spans, `foo_bar`, arrows, repeated punctuation, zero-width characters, and symbol-only inputs.

## Known Gaps

This change intentionally does not implement WeText-style semantic normalization for dates, long numbers, measurements, currency, or language-specific reading of every technical symbol. URL and path pronunciation remains a concise readability heuristic rather than a full domain-aware verbalizer.
