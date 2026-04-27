# Frontend VAD Capability Specification

## ADDED Requirements

### Requirement: Initialize VAD engine
The system SHALL initialize the ten-vad WebAssembly module with configurable parameters when the user enables voice recording.

#### Scenario: Successful VAD initialization
- **WHEN** user clicks "Start Listening" button
- **THEN** system SHALL load ten-vad WASM module
- **AND** system SHALL create VAD instance with hop_size=256, threshold=0.5
- **AND** system SHALL request microphone access
- **AND** system SHALL display "Listening" status

#### Scenario: VAD initialization failure
- **WHEN** WASM module fails to load or microphone access is denied
- **THEN** system SHALL display error message
- **AND** system SHALL provide retry button

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

### Requirement: Encode audio to WAV format
The system SHALL encode recorded audio buffer to WAV format (PCM, 16kHz, mono, 16-bit) before sending to backend.

#### Scenario: Successful WAV encoding
- **WHEN** recording stops and audio buffer contains data
- **THEN** system SHALL encode buffer to WAV format
- **AND** WAV SHALL have 16kHz sample rate
- **AND** WAV SHALL be mono (1 channel)
- **AND** WAV SHALL use 16-bit PCM encoding
- **AND** system SHALL include proper WAV header (44 bytes)

#### Scenario: Handle empty buffer
- **WHEN** recording stops but buffer is empty
- **THEN** system SHALL NOT attempt encoding
- **AND** system SHALL return to IDLE state

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

### Requirement: Handle memory management
The system SHALL properly allocate and deallocate WASM memory to prevent leaks.

#### Scenario: Allocate memory for VAD processing
- **WHEN** processing each audio frame
- **THEN** system SHALL allocate memory for audio data, probability, and flag pointers
- **AND** system SHALL copy audio frame to WASM heap

#### Scenario: Deallocate memory after processing
- **WHEN** audio frame processing completes
- **THEN** system SHALL free allocated memory for audio, probability, and flag pointers
- **AND** system SHALL repeat for each frame

#### Scenario: Cleanup on component unmount
- **WHEN** VoiceRecorder component unmounts
- **THEN** system SHALL destroy VAD instance
- **AND** system SHALL free VAD handle pointer
- **AND** system SHALL close audio context
- **AND** system SHALL release microphone stream

### Requirement: Configure VAD parameters
The system SHALL allow configurable VAD parameters for tuning and optimization.

#### Scenario: Use default parameters
- **WHEN** system initializes VAD
- **THEN** system SHALL use hop_size=256 (16ms @ 16kHz)
- **AND** system SHALL use threshold=0.5 (balanced detection)
- **AND** system SHALL use silence_frames=30 (480ms)

#### Scenario: Support parameter customization (future)
- **WHEN** user or developer provides custom parameters
- **THEN** system MAY accept custom threshold value (0.0-1.0)
- **AND** system MAY accept custom silence duration
- **AND** system SHALL validate parameter ranges
