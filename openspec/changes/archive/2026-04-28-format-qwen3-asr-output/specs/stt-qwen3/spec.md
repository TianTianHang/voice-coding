## ADDED Requirements

### Requirement: Parse Qwen3 ASR output metadata

The Qwen3 ASR engine SHALL parse decoded model output before returning `SttResult`, separating model metadata from user-visible transcript text.

#### Scenario: Automatic language output with metadata

- **WHEN** Qwen3-ASR decodes output containing `language <label><asr_text><text>` and no language was forced in `SttConfig`
- **THEN** `SttResult.text` SHALL contain only `<text>` trimmed of surrounding whitespace
- **AND** `SttResult.language` SHALL contain the normalized language represented by `<label>`

#### Scenario: Automatic language output with newline metadata

- **WHEN** Qwen3-ASR decodes output where the language metadata and `<asr_text>` tag are separated by newlines
- **THEN** the engine SHALL still extract the language metadata
- **AND** `SttResult.text` SHALL contain only the text after `<asr_text>` trimmed of surrounding whitespace

#### Scenario: Automatic language output without metadata

- **WHEN** Qwen3-ASR decodes output without an `<asr_text>` tag and no language was forced in `SttConfig`
- **THEN** `SttResult.text` SHALL contain the trimmed decoded output
- **AND** `SttResult.language` SHALL remain `"auto"`

#### Scenario: Forced language output

- **WHEN** `SttConfig.language` is set
- **THEN** the engine SHALL treat decoded model output as transcript text only
- **AND** `SttResult.text` SHALL contain the trimmed decoded output
- **AND** `SttResult.language` SHALL equal the configured language

#### Scenario: Empty audio metadata

- **WHEN** Qwen3-ASR decodes output containing `language None<asr_text>` with no text after the tag
- **THEN** `SttResult.text` SHALL be empty
- **AND** `SttResult.language` SHALL remain `"auto"`

#### Scenario: Empty audio metadata with returned text

- **WHEN** Qwen3-ASR decodes output containing `language None<asr_text><text>` with non-empty text after the tag
- **THEN** `SttResult.text` SHALL contain `<text>` trimmed of surrounding whitespace
- **AND** `SttResult.language` SHALL remain `"auto"`
