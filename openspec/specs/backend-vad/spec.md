# Spec: Backend Voice Activity Detection

## ADDED Requirements

### Requirement: Load ten-vad native library via FFI

The system SHALL dynamically load the ten-vad shared library using libloading.

#### Scenario: Library discovery

- **WHEN** VAD engine is initialized
- **THEN** system SHALL search for `libten_vad.so` at `libs/Linux/x64/libten_vad.so`
- **AND** it SHALL verify file exists
- **AND** if file missing, it SHALL return error with expected path

#### Scenario: Symbol resolution

- **WHEN** loading library
- **THEN** system SHALL resolve symbol `ten_vad_create`
- **AND** it SHALL resolve symbol `ten_vad_process`
- **AND** it SHALL resolve symbol `ten_vad_destroy`
- **AND** if any symbol missing, it SHALL return error

#### Scenario: Library lifetime

- **WHEN** VAD engine is dropped
- **THEN** system SHALL call `ten_vad_destroy` to release resources
- **AND** it SHALL unload the library automatically via RAII

### Requirement: Initialize VAD engine with parameters

The system SHALL initialize ten-vad engine with hop size and threshold parameters.

#### Scenario: Engine creation

- **WHEN** creating VAD engine
- **THEN** system SHALL call `ten_vad_create(hop_size=256, threshold=0.5)`
- **AND** it SHALL receive non-null handle pointer
- **AND** if handle is null, it SHALL return error "VAD initialization failed"

#### Scenario: Parameter specification

- **WHEN** initializing VAD engine
- **THEN** hop_size SHALL be 256 samples (16ms @ 16kHz)
- **AND** threshold SHALL be 0.5 (speech probability cutoff)
- **AND** these values match frontend WASM configuration

### Requirement: Process audio frames for speech detection

The system SHALL analyze each audio frame to detect speech activity.

#### Scenario: Frame processing

- **WHEN** audio frame of 256 i16 samples is received
- **THEN** system SHALL call `ten_vad_process(handle, audio, hop_size, &prob, &flag)`
- **AND** it SHALL receive probability score [0.0, 1.0]
- **AND** it SHALL receive binary flag (0 = non-speech, 1 = speech)
- **AND** if return code != 0, it SHALL return error

#### Scenario: Speech detection

- **WHEN** probability score exceeds threshold (0.5)
- **THEN** flag SHALL be 1 (speech detected)
- **AND** system SHALL trigger state transition to `recording`

#### Scenario: Non-speech detection

- **WHEN** probability score is below threshold (0.5)
- **THEN** flag SHALL be 0 (non-speech detected)
- **AND** if in `recording` state, system SHALL increment silence counter

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

### Requirement: Buffer audio during recording

The system SHALL accumulate audio frames while in `recording` state.

#### Scenario: Buffer management

- **WHEN** state transitions to `recording`
- **THEN** system SHALL clear existing buffer
- **AND** it SHALL allocate new buffer with capacity for 30 seconds (480000 samples)

#### Scenario: Frame accumulation

- **WHEN** in `recording` state and audio frame arrives
- **THEN** system SHALL extend buffer with frame data
- **AND** it SHALL check total length against MAX_RECORDING_SECONDS limit
- **AND** if limit exceeded, system SHALL truncate to 30 seconds

#### Scenario: Buffer retrieval

- **WHEN** silence threshold is reached and state transitions to `processing`
- **THEN** system SHALL clone complete buffer
- **AND** it SHALL clear the buffer for next recording
- **AND** it SHALL pass buffer to ASR transcription

### Requirement: Detect speech end via silence threshold

The system SHALL detect end of speech when consecutive non-speech frames exceed threshold and resume listening after the transcript is handled.

#### Scenario: Silence counting

- **WHEN** in `recording` state and non-speech frame is detected
- **THEN** system SHALL increment silence counter by 1

- **WHEN** in `recording` state and speech frame is detected
- **THEN** system SHALL reset silence counter to 0

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

### Requirement: Integrate with cpal audio stream

The system shall process audio frames from cpal callback through VAD engine.

#### Scenario: Real-time processing

- **WHEN** cpal delivers audio frame to callback
- **THEN** system SHALL pass frame to `VadEngine::process()`
- **AND** it SHALL receive (probability, is_speech) result
- **AND** it SHALL update state machine based on result
- **AND** callback SHALL return quickly (<1ms processing time)

#### Scenario: Thread safety

- **WHEN** audio callback is executing
- **THEN** VAD engine access SHALL be thread-safe
- **AND** state machine updates SHALL be synchronized via mutex
- **AND** mutex hold time SHALL be minimal (only during update)
