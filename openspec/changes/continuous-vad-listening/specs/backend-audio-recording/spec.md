# Spec: Backend Audio Recording

## MODIFIED Requirements

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
