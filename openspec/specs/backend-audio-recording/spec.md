# Spec: Backend Audio Recording

## ADDED Requirements

### Requirement: Access system microphone via cpal

The system SHALL capture audio from the default system microphone using the cpal library.

#### Scenario: Initialize audio stream

- **WHEN** `start_listening` command is invoked
- **THEN** system SHALL get default input device from cpal
- **AND** it SHALL configure stream with 16kHz sample rate
- **AND** it SHALL configure mono (1 channel)
- **AND** it SHALL configure i16 sample format
- **AND** it SHALL configure buffer size of 256 samples (16ms @ 16kHz)
- **AND** it SHALL start the audio stream

#### Scenario: Audio stream error handling

- **WHEN** no input device is available
- **THEN** system SHALL return error "No input device found"
- **AND** it SHALL send `error` event to frontend

- **WHEN** device is in use by another application
- **THEN** system SHALL return error with device name
- **AND** it SHALL suggest closing other applications

### Requirement: Receive audio frames in callback

The system SHALL process audio frames through cpal's input stream callback while the listening session remains active across multiple utterances.

#### Scenario: Frame delivery

- **WHEN** audio stream is active
- **THEN** cpal SHALL deliver frames of 256 i16 samples
- **AND** callback SHALL execute on cpal's audio thread
- **AND** frame rate SHALL be ~62.5 fps (16000 / 256)

#### Scenario: Continuous streaming

- **WHEN** application is in `listening` or `recording` state
- **THEN** callback SHALL continue receiving frames
- **AND** callback SHALL NOT block or delay audio processing

### Requirement: Auto-start stream on command

The system SHALL automatically start the audio stream when `start_listening` is called and keep it alive until explicit stop.

#### Scenario: Stream lifecycle

- **WHEN** `start_listening` command is invoked
- **THEN** system SHALL initialize cpal stream
- **AND** it SHALL call `stream.play()`
- **AND** frames SHALL start flowing to callback

#### Scenario: Stop stream

- **WHEN** `stop_listening` command is invoked
- **THEN** system SHALL drop the stream
- **AND** cpal SHALL automatically release device
- **AND** frame callbacks SHALL stop

### Requirement: Linux x64 platform support

The system SHALL support audio recording on Linux x64 platform.

#### Scenario: Device enumeration

- **WHEN** running on Linux x64
- **THEN** system SHALL use cpal's ALSA backend
- **AND** it SHALL enumerate available input devices via `cpal::default_host()`

#### Scenario: Permission handling

- **WHEN** user lacks microphone permission
- **THEN** cpal SHALL return error during stream creation
- **AND** system SHALL translate error to user-friendly message
- **AND** message SHALL suggest checking system microphone permissions

### Requirement: Audio format compatibility

The system SHALL produce audio in format compatible with ten-vad VAD engine.

#### Scenario: Format specification

- **WHEN** audio frames are delivered to callback
- **THEN** sample rate SHALL be 16000 Hz
- **AND** channel count SHALL be 1 (mono)
- **AND** sample format SHALL be signed 16-bit integer (i16)
- **AND** frame size SHALL be 256 samples (16 milliseconds)

#### Scenario: Format validation

- **WHEN** configuring audio stream
- **THEN** system SHALL validate requested config is supported
- **AND** if device doesn't support 16kHz, system SHALL return error
- **AND** error message SHALL include required vs supported formats
