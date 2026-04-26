# Spec: VAD Recording Stability

## ADDED Requirements

### Requirement: Temporary audio artifacts are always cleaned up

The system SHALL remove temporary audio files created for transcription regardless of whether transcription succeeds or fails.

#### Scenario: Successful transcription

- **WHEN** a transcription request completes successfully
- **THEN** the temporary audio file SHALL be deleted before the command returns

#### Scenario: Failed transcription

- **WHEN** a transcription request fails at any point after the temporary file is created
- **THEN** the temporary audio file SHALL still be deleted
- **AND** the error SHALL be returned to the caller

### Requirement: Stop cancels the active recording session

The system SHALL treat stop as a hard session boundary that cancels active recording and prevents stale results from being emitted.

#### Scenario: Stop during listening

- **WHEN** the user stops listening while recording or transcribing is in progress
- **THEN** the active session SHALL transition to idle
- **AND** any in-flight transcription result from that session SHALL be ignored if it arrives later

#### Scenario: Restart after stop

- **WHEN** the user starts a new listening session after stopping the previous one
- **THEN** the new session SHALL use a fresh session identifier
- **AND** results from the previous session SHALL NOT be appended to the new session transcript

### Requirement: Frontend state mirrors the backend session state

The system SHALL use backend events as the source of truth for listening state, and the frontend SHALL not override backend state locally.

#### Scenario: Backend emits state change

- **WHEN** the backend emits a VAD state event
- **THEN** the frontend SHALL update its visible state to match the emitted backend state

#### Scenario: Manual stop request

- **WHEN** the user requests stop from the frontend
- **THEN** the frontend SHALL wait for the backend state event or command result instead of forcing an optimistic idle state

### Requirement: Transcript display is session scoped

The system SHALL scope transcript history to the current recording session so that prior sessions do not pollute the visible transcript.

#### Scenario: New session starts

- **WHEN** a new listening session starts
- **THEN** the visible transcript SHALL reset for that session unless the UI explicitly renders prior history separately

#### Scenario: Transcript event arrives

- **WHEN** a transcript event arrives for the active session
- **THEN** the frontend SHALL append it to the current session transcript
- **AND** it SHALL ignore transcript events that do not match the active session

### Requirement: Errors are displayed through one unified path

The system SHALL route recording and transcription errors through a single frontend error presentation path.

#### Scenario: Backend error event

- **WHEN** the backend emits an error event
- **THEN** the frontend SHALL show that error in the transcript/error area

#### Scenario: Permission or device failure

- **WHEN** the backend reports a microphone permission or device failure
- **THEN** the error message SHALL remain actionable and visible until the user retries or dismisses it
