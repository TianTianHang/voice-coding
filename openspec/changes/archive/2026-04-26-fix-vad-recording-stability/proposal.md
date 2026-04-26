## Why

The current backend VAD pipeline has several stability issues that can leak temporary files, let stale transcription results arrive after stopping, and let the frontend drift out of sync with the backend state. These issues make the recorder feel unreliable and will get worse as the feature is used more often.

## What Changes

- Tighten the backend VAD lifecycle so stopping a session reliably cancels in-flight work and prevents late results from being emitted.
- Ensure temporary audio artifacts are cleaned up on both success and failure paths.
- Make frontend state and transcript display session-aware so old results do not bleed into new listening sessions.
- Unify error and status propagation so the UI reflects the backend as the single source of truth.

## Capabilities

### New Capabilities
- `vad-recording-stability`: Stable session lifecycle for backend VAD recording, including cleanup, cancellation, state synchronization, and session-scoped transcript handling.

### Modified Capabilities
- `stt-qwen3`: Transcription invocation must clean up temporary audio artifacts even when inference fails.

## Impact

- `src-tauri/src/asr.rs`: temporary file handling for audio transcription.
- `src-tauri/src/vad_commands.rs`: session lifecycle, cancellation, and event emission.
- `src-tauri/src/vad/state_machine.rs`: state transitions and stop/finish semantics.
- `src/hooks/useBackendVAD.ts`: listener lifecycle and frontend state synchronization.
- `src/components/VoiceRecorder.tsx`: transcript session handling and display logic.
- `src/components/TranscriptDisplay.tsx`: unified error display behavior.
- `openspec/specs/stt-qwen3/spec.md`: clarify cleanup behavior around transcription failures.
