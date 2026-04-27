# Spec: Real-Time VAD Events

## ADDED Requirements

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

#### Scenario: Transcription timeout

- **WHEN** ASR transcription takes longer than 30 seconds
- **THEN** system SHALL emit `error` event with timeout message
- **AND** system SHALL abort transcription
- **AND** system SHALL return to `idle` state

### Requirement: Event-driven communication model

The system SHALL use unidirectional event stream from backend to frontend.

#### Scenario: Frontend subscription

- **WHEN** frontend component mounts
- **THEN** it SHALL call `listen("vad-state", callback)` for state updates
- **AND** it SHALL call `listen("transcript", callback)` for transcription results
- **AND** it SHALL call `listen("error", callback)` for error messages
- **AND** callbacks SHALL be invoked when events are received

#### Scenario: Frontend unsubscription

- **WHEN** frontend component unmounts
- **THEN** it SHALL call `unlisten()` for each event listener
- **AND** backend SHALL continue processing (events are fire-and-forget)

### Requirement: Multiple window support

The system SHALL broadcast events to all Tauri windows (for future multi-window UI).

#### Scenario: Event broadcasting

- **WHEN** event is emitted
- **THEN** system SHALL deliver to all windows listening to that event name
- **AND** windows MAY subscribe to different event subsets
- **AND** if no windows are listening, event SHALL be dropped silently

### Requirement: Event payload serialization

The system SHALL serialize event payloads to JSON for IPC transmission.

#### Scenario: State serialization

- **WHEN** emitting `vad-state` event
- **THEN** payload SHALL be state enum as string
- **AND** enum SHALL derive `serde::Serialize`
- **AND** values SHALL be "Idle", "Listening", "Recording", "Processing"

#### Scenario: Transcript serialization

- **WHEN** emitting `transcript` event
- **THEN** payload SHALL be JSON object with `text` field
- **AND** `text` value SHALL be String type
- **AND** special characters SHALL be properly escaped

#### Scenario: Error serialization

- **WHEN** emitting `error` event
- **THEN** payload SHALL be String (error message)
- **AND** message SHALL be human-readable
- **AND** technical details MAY be included for debugging

### Requirement: Event delivery guarantees

The system SHALL make best-effort attempt to deliver events but handle failures gracefully.

#### Scenario: Frontend not listening

- **WHEN** event is emitted and no frontend listener is registered
- **THEN** event SHALL be dropped
- **AND** backend SHALL continue processing normally
- **AND** no error SHALL be logged

#### Scenario: Frontend window closed

- **WHEN** event is being delivered and window closes
- **THEN** emit operation SHALL return error
- **AND** backend SHALL log warning
- **AND** backend SHALL continue processing (other windows unaffected)

#### Scenario: Serialization failure

- **WHEN** event payload cannot be serialized to JSON
- **THEN** backend SHALL log error
- **AND** backend SHALL skip emitting that event
- **AND** backend SHALL continue processing next events
