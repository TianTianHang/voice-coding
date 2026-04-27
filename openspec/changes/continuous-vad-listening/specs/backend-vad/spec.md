# Spec: Backend Voice Activity Detection

## MODIFIED Requirements

### Requirement: Maintain VAD state machine
The system SHALL manage state transitions between idle, listening, recording, and processing while keeping the session in listening mode after each utterance completes.

#### Scenario: State transitions
- **WHEN** VAD engine starts
- **THEN** initial state SHALL be `idle`

- **WHEN** `start_listening` command is received
- **THEN** state SHALL transition to `listening`
- **AND** system SHALL emit `vad-state` event with value "Listening"

- **WHEN** speech is detected in `listening` state
- **THEN** state SHALL transition to `recording`
- **AND** system SHALL start buffering audio frames
- **AND** system SHALL emit `vad-state` event with value "Recording"

- **WHEN** silence counter reaches threshold (30 frames = 480ms) in `recording` state
- **THEN** state SHALL transition to `processing`
- **AND** system SHALL emit `vad-state` event with value "Processing"
- **AND** system SHALL trigger ASR transcription

- **WHEN** transcription completes successfully
- **THEN** state SHALL transition to `listening`
- **AND** system SHALL emit `vad-state` event with value "Listening"

- **WHEN** transcription fails for a recoverable reason
- **THEN** state SHALL transition to `listening`
- **AND** system SHALL emit `vad-state` event with value "Listening"

- **WHEN** `stop_listening` command is received or a fatal device failure occurs
- **THEN** state SHALL transition to `idle`
- **AND** system SHALL emit `vad-state` event with value "Idle"

#### Scenario: State query
- **WHEN** `get_vad_state` command is invoked
- **THEN** system SHALL return current state as string
- **AND** possible values SHALL be "Idle", "Listening", "Recording", "Processing"

### Requirement: Detect speech end via silence threshold
The system SHALL detect end of speech when consecutive non-speech frames exceed threshold and resume listening after the transcript is handled.

#### Scenario: Threshold trigger
- **WHEN** silence counter reaches 30 consecutive frames (480ms)
- **THEN** system SHALL consider speech ended
- **AND** it SHALL transition to `processing` state
- **AND** it SHALL trigger ASR transcription

#### Scenario: Minimum recording duration
- **WHEN** total recording duration < 0.5 seconds (8000 samples)
- **THEN** system SHALL discard recording
- **AND** it SHALL return to `listening` state
- **AND** it SHALL NOT trigger transcription
