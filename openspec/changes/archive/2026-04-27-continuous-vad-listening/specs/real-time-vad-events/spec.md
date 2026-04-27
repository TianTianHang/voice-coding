# Spec: Real-Time VAD Events

## MODIFIED Requirements

### Requirement: Emit VAD state changes to frontend
The system SHALL push state transition events to frontend via Tauri event system and continue emitting `listening` after each completed utterance.

#### Scenario: State change events
- **WHEN** VAD state machine transitions to new state
- **THEN** system SHALL emit `vad-state` event
- **AND** event payload SHALL be state name as string
- **AND** possible values SHALL be "Idle", "Listening", "Recording", "Processing"
- **AND** event SHALL be delivered to all listening frontend windows

#### Scenario: Event delivery timing
- **WHEN** state transition occurs
- **THEN** event SHALL be emitted within 10ms
- **AND** frontend SHALL receive event within 100ms (including IPC)

### Requirement: Emit transcription results
The system SHALL push ASR transcript text to frontend when transcription completes and keep the listening session active afterwards.

#### Scenario: Successful transcription
- **WHEN** ASR transcription completes successfully
- **THEN** system SHALL emit `transcript` event
- **AND** event payload SHALL be JSON `{ "text": "<transcription result>" }`
- **AND** text SHALL be complete transcribed string (no partial results)

#### Scenario: Transcription error
- **WHEN** ASR transcription fails for a recoverable reason
- **THEN** system SHALL emit `error` event
- **AND** event payload SHALL include error message
- **AND** frontend SHALL display error to user
- **AND** system SHALL continue the listening session

### Requirement: Emit error events
The system SHALL push runtime errors to frontend for user feedback.

#### Scenario: Audio device errors
- **WHEN** audio device becomes unavailable during recording
- **THEN** system SHALL emit `error` event with message
- **AND** message SHALL include device name and error reason
- **AND** system SHALL transition to `idle` state

#### Scenario: VAD engine errors
- **WHEN** VAD processing fails (e.g., FFI call returns error)
- **THEN** system SHALL emit `error` event with message
- **AND** message SHALL specify which operation failed
- **AND** system SHALL transition to `idle` state
