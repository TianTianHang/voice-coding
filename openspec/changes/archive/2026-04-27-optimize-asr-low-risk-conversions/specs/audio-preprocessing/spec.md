## MODIFIED Requirements

### Requirement: Accept raw audio samples

The system SHALL accept pre-decoded float32 samples for advanced use cases.

#### Scenario: Raw samples at 16kHz

- **WHEN** user provides `AudioInput::Samples(vec, 16000)`
- **THEN** system SHALL use samples directly without decoding
- **AND** it SHALL skip format detection and resampling
- **AND** it SHALL preserve the provided sample values except for validation

#### Scenario: Raw samples from backend VAD

- **WHEN** backend VAD provides mono `i16` PCM samples at 16000 Hz
- **THEN** system SHALL convert them directly to float32 samples
- **AND** it SHALL pass them to ASR as `AudioInput::Samples(vec, 16000)`
- **AND** it SHALL bypass byte-buffer audio decoding

#### Scenario: Raw samples at different sample rate

- **WHEN** user provides samples at 48000 Hz
- **THEN** system SHALL resample to 16kHz
- **AND** it SHALL apply high-quality resampling algorithm

#### Scenario: Validate sample rate

- **WHEN** user provides samples at unsupported rate (< 8kHz or > 48kHz)
- **THEN** system SHALL return `AudioLoadError`
- **AND** error SHALL specify valid sample rate range
