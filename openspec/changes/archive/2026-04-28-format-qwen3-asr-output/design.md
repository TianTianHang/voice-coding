## Context

The Qwen3 ASR pipeline decodes generated tokens directly into `SttResult.text`. In automatic language mode, the model may include metadata in the decoded output, commonly in the form `language Chinese<asr_text>...`. That raw string is currently propagated through Tauri commands and backend VAD transcript events.

Forced-language mode already changes the prompt shape: the prompt builder pre-fills `language {lang}<asr_text>` when `SttConfig.language` is set, so the model is expected to generate transcription text only. Automatic mode leaves language detection to the model.

## Goals / Non-Goals

**Goals:**

- Parse decoded Qwen3-ASR output into clean transcript text and an output language value.
- Keep frontend code unchanged by ensuring backend results are cleaned before Tauri commands/events use them.
- Preserve existing auto-mode behavior when the model does not emit language metadata by keeping `SttResult.language` as `"auto"`.
- Preserve forced-language behavior by treating decoded output as transcript text only and returning the configured language.
- Cover parser behavior with deterministic unit tests that do not require model files.

**Non-Goals:**

- Change prompt structure, model files, tokenizer behavior, decoder stop-token logic, or ASR inference flow.
- Add frontend parsing or new frontend state for detected language.
- Change the public Tauri command return shape from `String` to a structured object.
- Add new runtime dependencies.

## Decisions

1. Parse output inside `stt-qwen3` immediately after tokenizer decode.

   The parser should run after `tokenizer.decode(&generated_tokens)` and before constructing `SttResult`. This keeps all callers consistent: file transcription, memory-buffer transcription, and backend VAD events all consume the same cleaned result.

   Alternative considered: parse in Tauri command handlers or frontend hooks. That duplicates model-specific behavior outside the model crate and leaves non-Tauri callers exposed to raw metadata.

2. Return parsed detected language only when metadata is present.

   In auto mode, tagged output such as `language Chinese<asr_text>text` should produce clean `text` and a normalized detected language. If no `<asr_text>` tag is present, the transcript should be the trimmed raw output and `SttResult.language` should remain `"auto"`.

   Alternative considered: mirror the reference Python helper exactly and return an empty language for no-tag output. That would change the Rust API's current auto-mode convention and provide less useful state to callers.

3. Treat forced language as text-only output.

   When `SttConfig.language` is set, the configured language should be returned and decoded output should be trimmed as transcript text. This matches the prompt behavior where the metadata prefix is already provided to the model.

   Alternative considered: parse tags even in forced-language mode. That risks stripping valid user text if the transcription happens to contain the marker and contradicts the forced-language prompt contract.

4. Keep language normalization local and conservative.

   The parser should normalize common Qwen3 language labels into the existing supported language codes where practical, for example `Chinese` to `zh`, `English` to `en`, and `Cantonese` to `yue`. Unknown non-empty labels may be returned trimmed or mapped to `"auto"` depending on implementation preference, but tests should lock down the selected behavior.

   Alternative considered: introduce a shared language registry in `stt-core`. That is broader than needed because this parser is specific to Qwen3 output formatting.

## Risks / Trade-offs

- Marker collision in transcript text -> Only auto mode interprets `<asr_text>` as metadata; forced-language mode treats output as text-only.
- Incomplete language label mapping -> Start with labels matching supported languages and add mappings as model behavior is observed.
- Empty-audio false positives -> Only treat `language None<asr_text>` with empty text as empty audio; if text exists after the tag, preserve the text and keep language as `"auto"`.
- Chunked audio language variation -> Existing chunk aggregation can keep using the overall configured/detected language behavior; text cleaning remains per chunk.
