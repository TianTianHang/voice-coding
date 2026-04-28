# Spec: Qwen3 ASR Implementation (Delta)

## MODIFIED Requirements

### Requirement: Transcribe audio from memory buffer

The system SHALL support transcribing audio data directly from memory buffers without requiring temporary files.

#### Scenario: In-memory audio transcription

- **WHEN** `transcribe_audio_data` is called with `Vec<u8>` WAV data
- **THEN** system SHALL parse WAV header and extract PCM samples
- **AND** it SHALL convert samples to required format (16kHz mono float32)
- **AND** it SHALL process audio through complete ASR pipeline
- **AND** it SHALL return `SttResult` with transcribed text
- **AND** it SHALL NOT create temporary files on disk

#### Scenario: WAV format validation

- **WHEN** `transcribe_audio_data` receives invalid WAV data
- **THEN** system SHALL return `AudioLoadError`
- **AND** error message SHALL specify validation failure reason
- **AND** possible reasons include: invalid header, unsupported format, corrupted data

#### Scenario: Buffer size limits

- **WHEN** audio buffer exceeds 30 seconds (480000 samples)
- **THEN** system SHALL process normally (no truncation)
- **AND** it MAY use VAD chunking if enabled in config

#### Scenario: Integration with backend VAD

- **WHEN** called from backend VAD's state machine
- **THEN** input buffer SHALL be `Vec<i16>` raw PCM samples
- **AND** system SHALL convert i16 to f32 (divide by 32768.0)
- **AND** it SHALL call the ASR engine through `AudioInput::Samples` with sample rate 16000
- **AND** it SHALL NOT encode the raw samples as WAV bytes
- **AND** it SHALL NOT decode the VAD audio with Symphonia
- **AND** it SHALL perform Mel spectrogram computation
- **AND** it SHALL return transcription result as String
- **AND** result SHALL be emitted via `transcript` event to frontend

#### Scenario: Performance comparison

- **WHEN** comparing backend VAD transcription before and after this change
- **THEN** the optimized path SHALL remove WAV container construction and generic byte-buffer decoding overhead
- **AND** both versions SHALL produce equivalent transcription results for the same raw PCM samples
- **AND** memory usage SHALL be lower for the VAD path because it avoids the intermediate WAV byte buffer

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
