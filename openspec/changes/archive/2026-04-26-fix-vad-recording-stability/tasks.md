## 1. Backend session lifecycle

- [x] 1.1 Add explicit session identity to backend VAD/transcription events
- [x] 1.2 Make `stop_listening` invalidate the active session before returning
- [x] 1.3 Ensure late transcription results from old sessions are ignored
- [x] 1.4 Verify the state machine transitions to idle only after session teardown is complete

## 2. Transcription cleanup

- [x] 2.1 Move temporary audio file cleanup into an unconditional cleanup path
- [x] 2.2 Preserve the original transcription error while still removing temp files
- [x] 2.3 Verify temp files are removed on both success and failure paths

## 3. Frontend state synchronization

- [x] 3.1 Remove render-time transcript history mutation from `VoiceRecorder`
- [x] 3.2 Scope transcript display to the active session
- [x] 3.3 Stop forcing local idle state on stop requests and wait for backend events
- [x] 3.4 Route backend errors through one unified UI path

## 4. Listener and event safety

- [x] 4.1 Make `useBackendVAD` listener registration and cleanup deterministic
- [x] 4.2 Prevent duplicate event subscriptions under React StrictMode
- [x] 4.3 Verify that stale events are ignored after unmount or stop

## 5. Documentation and verification

- [x] 5.1 Update OpenSpec and architecture notes to describe session ownership and cleanup
- [x] 5.2 Add or update tests for stop/cancel behavior and temp file cleanup
- [x] 5.3 Run targeted backend and frontend builds/tests to confirm stability fixes
