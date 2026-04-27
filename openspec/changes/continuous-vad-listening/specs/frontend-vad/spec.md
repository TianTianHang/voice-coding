# Spec: Frontend VAD Continuous Listening

## MODIFIED Requirements

### Requirement: Provide real-time status feedback
The system SHALL display current VAD state and recording status to the user while preserving a continuous listening session between utterances.

#### Scenario: Display listening status
- **WHEN** system is initialized and waiting for the next utterance in an active session
- **THEN** system SHALL display "🎤 Listening..." or equivalent waiting status
- **AND** indicator SHALL use a calming color (e.g., blue)
- **AND** the UI SHALL communicate that the session remains active

#### Scenario: Display recording status
- **WHEN** system is in RECORDING state
- **THEN** system SHALL display "🔴 Recording..." indicator
- **AND** indicator SHALL use active color (e.g., red)
- **AND** system MAY display audio duration

#### Scenario: Display processing status
- **WHEN** system is in PROCESSING state
- **THEN** system SHALL display "⏳ Processing..." indicator
- **AND** system SHALL disable recording controls or mark them as unavailable while transcription runs

#### Scenario: Display error status
- **WHEN** error occurs (VAD failure, microphone denied, transcription failure, etc.)
- **THEN** system SHALL display error message
- **AND** system SHALL provide guidance for resolution when possible

### Requirement: Process audio frames with VAD
The system SHALL continuously process audio frames through VAD to detect voice activity across multiple utterances in one session.

#### Scenario: Detect speech start
- **WHEN** VAD returns is_speech=1 for current frame
- **AND** current state is LISTENING
- **THEN** system SHALL transition to RECORDING state
- **AND** system SHALL start recording audio frames to buffer

#### Scenario: Detect speech end
- **WHEN** VAD returns is_speech=0 for 30 consecutive frames (480ms)
- **AND** current state is RECORDING
- **THEN** system SHALL stop recording
- **AND** system SHALL transition to PROCESSING state
- **AND** system SHALL trigger transcription of recorded audio

#### Scenario: Continue recording during speech
- **WHEN** VAD returns is_speech=1
- **AND** current state is RECORDING
- **THEN** system SHALL continue appending audio frames to buffer
- **AND** system SHALL reset silence counter

#### Scenario: Brief silence during speech
- **WHEN** VAD returns is_speech=0 for less than 30 frames
- **AND** current state is RECORDING
- **THEN** system SHALL continue recording
- **AND** system SHALL NOT stop recording

### Requirement: Maintain continuous listening session
The system SHALL return to LISTENING after each successful or recoverable transcription so the user can speak again without restarting the session.

#### Scenario: Resume listening after transcription
- **WHEN** transcription is completed successfully
- **THEN** system SHALL transition to LISTENING state
- **AND** system SHALL keep the listening session active
- **AND** system SHALL remain ready to detect the next utterance

#### Scenario: Resume listening after recoverable transcription error
- **WHEN** transcription fails for a recoverable reason
- **THEN** system SHALL display an error message
- **AND** system SHALL transition to LISTENING state
- **AND** system SHALL continue the current listening session

### Requirement: Manage audio buffer
The system SHALL maintain an in-memory buffer for recorded audio with configurable maximum duration.

#### Scenario: Buffer audio during recording
- **WHEN** system is in RECORDING state
- **THEN** system SHALL append each audio frame to buffer
- **AND** system SHALL limit buffer to maximum 30 seconds
- **AND** system SHALL discard oldest frames if buffer exceeds limit

#### Scenario: Clear buffer after transcription
- **WHEN** transcription is completed
- **THEN** system SHALL clear audio buffer
- **AND** system SHALL transition to LISTENING state
