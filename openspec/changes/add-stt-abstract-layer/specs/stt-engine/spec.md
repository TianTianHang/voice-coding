# Spec: STT Engine Abstraction

## ADDED Requirements

### Requirement: SttEngine trait defines common interface

The system SHALL provide a `SttEngine` trait that all speech-to-text implementations MUST implement, defining a common interface for transcription operations.

#### Scenario: Trait includes required methods

- **WHEN** a type implements the `SttEngine` trait
- **THEN** it MUST provide `engine_name()` method returning engine identifier
- **AND** it MUST provide `supported_languages()` method returning language codes
- **AND** it MUST provide `transcribe()` method for single-file transcription
- **AND** it MUST provide `health_check()` method for availability verification

#### Scenario: Trait supports async operations

- **WHEN** calling `transcribe()` method
- **THEN** the method SHALL be async (return `Future`)
- **AND** it MUST accept `AudioInput` enum as source
- **AND** it MUST accept `SttConfig` for configuration
- **AND** it MUST return `SttResult` with transcription text and metadata

### Requirement: Audio input supports multiple formats

The system SHALL accept audio input through the `AudioInput` enum supporting file paths, byte buffers, and raw samples.

#### Scenario: File path input

- **WHEN** user provides audio file path
- **THEN** system SHALL load audio from specified path
- **AND** it MUST support common formats (WAV, MP3, FLAC, OGG, M4A)

#### Scenario: Byte buffer input

- **WHEN** user provides audio as byte buffer
- **THEN** system SHALL decode audio from memory
- **AND** it MUST avoid temporary file creation

#### Scenario: Raw samples input

- **WHEN** user provides pre-decoded float samples
- **THEN** system SHALL accept samples with specified sample rate
- **AND** it MUST validate sample rate is 16kHz (or resample)

### Requirement: Configuration allows runtime customization

The system SHALL provide `SttConfig` struct for customizing transcription behavior at runtime.

#### Scenario: Language selection

- **WHEN** user specifies language in config (e.g., `"en"`, `"zh"`)
- **THEN** system SHALL use that language for transcription
- **AND** it MUST return error if language is not supported

#### Scenario: VAD chunking enable/disable

- **WHEN** user sets `enable_vad: true`
- **THEN** system SHALL automatically split long audio at silence boundaries
- **AND** default chunk duration SHALL be 30 seconds

#### Scenario: Custom chunk duration

- **WHEN** user specifies `chunk_seconds: Some(20)`
- **THEN** system SHALL use 20-second target for VAD splitting
- **AND** it MUST accept range 5-60 seconds

#### Scenario: Max tokens limit

- **WHEN** user specifies `max_new_tokens: Some(256)`
- **THEN** decoder SHALL stop after generating 256 tokens
- **AND** it MUST still respect end-of-sequence tokens

### Requirement: Result includes transcription and metadata

The system SHALL return `SttResult` containing transcribed text, detected language, and timing information.

#### Scenario: Successful transcription

- **WHEN** transcription completes successfully
- **THEN** result SHALL contain `text` field with transcribed content
- **AND** it SHALL contain `language` field with detected/specified language
- **AND** it SHALL contain `timing` with audio duration and processing time
- **AND** it SHALL calculate RTF (real-time factor = processing_time / audio_duration)

#### Scenario: Timing information

- **WHEN** result includes `timing` field
- **THEN** it SHALL contain `audio_duration_sec` (input audio length)
- **AND** it SHALL contain `processing_time_sec` (wall-clock time)
- **AND** it SHALL contain `rtf` (ratio < 1.0 means faster than realtime)
- **AND** it MAY contain `tokens_generated` (number of output tokens)

### Requirement: Error handling is comprehensive

The system SHALL define `SttError` enum covering all failure modes with clear error messages.

#### Scenario: Audio loading errors

- **WHEN** audio file cannot be loaded
- **THEN** system SHALL return `AudioLoadError` with descriptive message
- **AND** it MUST specify format compatibility issues if applicable

#### Scenario: Inference errors

- **WHEN** ONNX model inference fails
- **THEN** system SHALL return `InferenceError` with details
- **AND** it MUST include model name and operation that failed

#### Scenario: Unsupported language

- **WHEN** user requests unsupported language
- **THEN** system SHALL return `UnsupportedLanguage` with language code
- **AND** it MAY list supported languages in error message

#### Scenario: Feature not implemented

- **WHEN** calling optional feature (e.g., streaming) not supported by engine
- **THEN** system SHALL return `NotImplemented` error
- **AND** it MUST not panic or abort

### Requirement: Compile-time engine selection

The system SHALL use Rust feature flags for compile-time engine selection, enabling zero-overhead abstraction.

#### Scenario: Default engine selection

- **WHEN** building with default features
- **THEN** system SHALL compile `stt-qwen3` engine
- **AND** it SHALL set `CurrentSttEngine` type alias to `Qwen3AsrEngine`

#### Scenario: Custom engine selection

- **WHEN** building with `--features stt-whisper`
- **THEN** system SHALL compile only `stt-whisper` engine
- **AND** unused engines SHALL NOT be included in binary

#### Scenario: Type alias for current engine

- **WHEN** code references `CurrentSttEngine`
- **THEN** it SHALL resolve to selected engine type
- **AND** calling methods SHALL be statically dispatched (no vtable overhead)

### Requirement: Streaming STT is optional capability

The system SHALL provide `StreamingStt` marker trait for engines that support real-time audio streaming.

#### Scenario: Streaming trait extends base trait

- **WHEN** engine implements `StreamingStt`
- **THEN** it MUST also implement `SttEngine`
- **AND** it SHALL provide `transcribe_stream()` method

#### Scenario: Non-streaming engines

- **WHEN** engine does NOT implement `StreamingStt`
- **THEN** calling `transcribe_stream()` SHALL return `NotImplemented` error
- **AND** it MUST not require streaming support

### Requirement: Batch processing is optional capability

The system SHALL provide `BatchStt` marker trait for engines that support optimized batch transcription.

#### Scenario: Batch trait extends base trait

- **WHEN** engine implements `BatchStt`
- **THEN** it MUST provide optimized `transcribe_batch()` method
- **AND** it MAY process files in parallel

#### Scenario: Default batch implementation

- **WHEN** base `SttEngine` trait's `transcribe_batch()` is called
- **THEN** it SHALL process files sequentially using `transcribe()`
- **AND** specialized engines MAY override for better performance

### Requirement: Health check verifies engine availability

The system SHALL provide `health_check()` method to verify engine is ready for inference.

#### Scenario: Successful health check

- **WHEN** `health_check()` is called and engine is ready
- **THEN** it SHALL return `Ok(true)`
- **AND** it MUST verify model files are present
- **AND** it MUST verify tokenizer is loaded

#### Scenario: Health check failure

- **WHEN** engine has missing dependencies
- **THEN** `health_check()` SHALL return `Err(SttError)`
- **AND** error MUST specify what is missing (models, tokenizer, etc.)

### Requirement: All trait methods are thread-safe

The system SHALL require `Send + Sync` bounds for `SttEngine` trait, enabling safe concurrent use.

#### Scenario: Concurrent transcription requests

- **WHEN** multiple async tasks call `transcribe()` simultaneously
- **THEN** engine MUST handle concurrent requests safely
- **AND** it SHALL not race on internal state
- **AND** each request SHALL get correct result

#### Scenario: Shared engine instance

- **WHEN** engine is wrapped in `Arc` for sharing
- **THEN** it MUST implement `Sync`
- **AND** immutable operations SHALL be lock-free where possible
