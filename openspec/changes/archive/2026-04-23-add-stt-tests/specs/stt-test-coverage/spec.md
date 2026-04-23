# Spec: STT Test Coverage

Comprehensive test coverage requirements for the STT inference pipeline to ensure reliability and prevent regressions.

## ADDED Requirements

### Requirement: Decoder module comprehensive testing
The system SHALL provide complete unit test coverage for all decoder functions including autoregressive generation, KV cache management, and embedding fusion.

#### Scenario: Embed and fuse audio with text tokens
- **WHEN** audio encoder output and prompt tokens with audio pad tokens are provided
- **THEN** system SHALL correctly fuse encoder embeddings at audio pad positions
- **AND** SHALL validate that audio pad count matches encoder output count
- **AND** SHALL return error when counts mismatch

#### Scenario: Decoder initialization generates first token
- **WHEN** fused embeddings are provided to decoder_init
- **THEN** system SHALL run decoder_init ONNX session
- **AND** SHALL extract first token using greedy decoding from logits
- **AND** SHALL populate KV cache with present keys/values
- **AND** SHALL return token ID and initialized cache

#### Scenario: Single autoregressive decoding step
- **WHEN** a token ID, position, and cache are provided to decoder_step
- **THEN** system SHALL embed the token using embedding matrix
- **AND** SHALL run decoder_step ONNX session with past KV cache
- **AND** SHALL extract next token using greedy decoding
- **AND** SHALL return new token and updated cache

#### Scenario: Autoregressive generation stops correctly
- **WHEN** run_autoregressive_decode generates tokens up to max_tokens
- **THEN** system SHALL stop early if IM_END_ID or ENDOFTEXT_ID is generated
- **AND** SHALL return all generated tokens including the final token
- **AND** SHALL not exceed max_new_tokens limit

#### Scenario: Greedy decoding selects highest probability token
- **WHEN** logits tensor is provided
- **THEN** system SHALL select token with maximum logit value
- **AND** SHALL return token ID as u32

### Requirement: Fast unit tests with mocked ONNX sessions
The system SHALL provide mock implementations of ONNX sessions to enable fast unit tests without loading real models.

#### Scenario: Mock encoder sessions produce deterministic outputs
- **WHEN** encoder tests run with mock sessions
- **THEN** tests SHALL complete in under 100ms
- **AND** SHALL produce deterministic tensor outputs for given inputs
- **AND** SHALL not require model files to be present

#### Scenario: Mock decoder sessions simulate cache growth
- **WHEN** decoder step tests run with mock sessions
- **THEN** system SHALL simulate KV cache growth across steps
- **AND** SHALL return plausible logits for token generation
- **AND** SHALL validate input tensor shapes match ONNX model expectations

### Requirement: Prompt builder property-based testing
The system SHALL provide property-based tests for prompt building to ensure invariants hold across wide range of inputs.

#### Scenario: Prompt structure invariants
- **WHEN** build_prompt_ids is called with any valid n_audio_tokens and language
- **THEN** output SHALL always start with IM_START_ID
- **AND** SHALL contain exactly n_audio_tokens AUDIO_PAD_ID tokens
- **AND** SHALL have matching IM_START_ID and IM_END_ID pairs
- **AND** SHALL end with language tokens if language is provided

#### Scenario: Audio token count preservation
- **WHEN** build_prompt_ids is called with n_audio_tokens = N
- **THEN** output SHALL contain exactly N AUDIO_PAD_ID tokens between AUDIO_START_ID and AUDIO_END_ID

#### Scenario: Special token constants validity
- **WHEN** special token constants are used
- **THEN** IM_START_ID, IM_END_ID, ENDOFTEXT_ID SHALL be valid token IDs
- **AND** AUDIO_START_ID, AUDIO_END_ID, AUDIO_PAD_ID SHALL be valid token IDs
- **AND** constants SHALL match Qwen tokenizer specification

### Requirement: Tokenizer round-trip testing
The system SHALL provide round-trip tests to ensure tokenizer encode/decode correctness across various text types.

#### Scenario: Simple text round-trip
- **WHEN** text is encoded then decoded
- **THEN** decoded text SHALL match original text for ASCII characters
- **AND** SHALL handle spaces and punctuation correctly

#### Scenario: Multilingual text round-trip
- **WHEN** text in supported languages (zh, en, yue, ja, ko, etc.) is encoded then decoded
- **THEN** decoded text SHALL match original text
- **AND** SHALL preserve language-specific characters

#### Scenario: Special character handling
- **WHEN** text with special characters, newlines, or emojis is processed
- **THEN** tokenizer SHALL handle without errors
- **AND** round-trip SHALL preserve character semantics where possible

#### Scenario: Missing tokenizer error
- **WHEN** tokenizer file is not found
- **THEN** load SHALL return clear error message
- **AND** error SHALL include the missing file path

### Requirement: Encoder edge case testing
The system SHALL provide tests for encoder edge cases including chunking, empty inputs, and boundary conditions.

#### Scenario: Mel spectrogram chunking boundaries
- **WHEN** mel spectrogram with frames not divisible by CHUNK_SIZE is chunked
- **THEN** last chunk SHALL contain remaining frames
- **AND** original_lengths SHALL accurately record each chunk's actual length
- **AND** chunks SHALL be padded to max_chunk_len

#### Scenario: Small input handling
- **WHEN** audio with fewer than CHUNK_SIZE frames is processed
- **THEN** system SHALL return single chunk
- **AND** SHALL not apply chunking logic

#### Scenario: Conv output length calculation
- **WHEN** compute_conv_output_len is called with various input lengths
- **THEN** output SHALL match expected 3-layer conv stride-2 reduction
- **AND** SHALL be deterministic for same input

### Requirement: Error path testing in transcription pipeline
The system SHALL test error conditions in the main transcription flow.

#### Scenario: Invalid language rejection
- **WHEN** transcribe is called with unsupported language code
- **THEN** system SHALL return UnsupportedLanguage error
- **AND** SHALL not attempt transcription

#### Scenario: Audio validation failures
- **WHEN** audio that is too short (< 0.1s) is provided
- **THEN** system SHALL return AudioLoadError
- **AND** error message SHALL indicate minimum duration requirement

#### Scenario: Missing model files
- **WHEN** required model files are missing
- **THEN** health_check SHALL return InferenceError
- **AND** error SHALL list specific missing files

### Requirement: Test coverage thresholds
The system SHALL achieve minimum code coverage thresholds across all STT modules.

#### Scenario: Decoder module coverage
- **WHEN** coverage is measured for decoder.rs
- **THEN** line coverage SHALL be >= 90%
- **AND** all public functions SHALL have tests

#### Scenario: Prompt builder coverage
- **WHEN** coverage is measured for prompt.rs
- **THEN** line coverage SHALL be >= 95%
- **AND** property tests SHALL cover edge cases

#### Scenario: Tokenizer coverage
- **WHEN** coverage is measured for tokenizer/wrapper.rs
- **THEN** line coverage SHALL be >= 90%
- **AND** encode/decode round-trips SHALL be tested

#### Scenario: Overall crate coverage
- **WHEN** coverage is measured for stt-qwen3 crate
- **THEN** overall line coverage SHALL be >= 85%
- **AND** all critical paths SHALL have tests

### Requirement: Test fixtures and helpers
The system SHALL provide reusable test utilities to reduce duplication across tests.

#### Scenario: Mock session builders
- **WHEN** tests need mock ONNX sessions
- **THEN** helpers SHALL provide builders for common mock configurations
- **AND** SHALL support customizing output shapes and values

#### Scenario: Test data generators
- **WHEN** tests need audio samples or mel spectrograms
- **THEN** fixtures SHALL provide deterministic test data
- **AND** SHALL support various sizes and edge cases

#### Scenario: Assertion helpers
- **WHEN** tests validate complex structures (KV cache, embeddings)
- **THEN** helpers SHALL provide assertion macros
- **AND** SHALL provide clear failure messages
