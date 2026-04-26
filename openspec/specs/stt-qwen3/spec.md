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
- **THEN** input buffer SHALL be Vec<i16> raw PCM samples
- **AND** system SHALL convert i16 to f32 (divide by 32768.0)
- **AND** it SHALL perform Mel spectrogram computation
- **AND** it SHALL return transcription result as String
- **AND** result SHALL be emitted via `transcript` event to frontend

#### Scenario: Performance comparison

- **WHEN** comparing `transcribe_audio_data` vs `transcribe` (file path)
- **THEN** memory buffer version SHALL be ~10% faster (no disk I/O)
- **AND** both versions SHALL produce identical transcription results
- **AND** memory usage SHALL be comparable ( WAV buffer ~2x file size)
