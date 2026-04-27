## Why

The current voice flow ends each transcription by returning to `idle`, which makes the UI look like recording has fully stopped even though the user may want to keep speaking. We want a continuous listening experience so the app stays ready for the next utterance after each transcription finishes.

## What Changes

- Keep the voice session active after a transcript is produced instead of ending the flow at `idle`.
- Change the post-transcription state transition from `processing -> idle` to `processing -> listening`.
- Preserve `idle` as the explicit stopped state used only when the user stops listening or a fatal device error occurs.
- Keep emitting transcript and error events per utterance while allowing multiple utterances in a single listening session.
- Update the frontend state presentation so the app communicates that it is still listening between utterances.

## Capabilities

### New Capabilities
- None

### Modified Capabilities
- `backend-vad`: VAD state machine and session behavior change from single-shot flow to continuous listening.
- `frontend-vad`: Frontend state handling and status presentation must reflect continuous listening semantics.
- `real-time-vad-events`: Event-driven state updates must preserve the listening session across transcriptions.
- `backend-audio-recording`: Audio stream lifecycle must support a long-lived listening session rather than a one-shot cycle.

## Impact

- Rust backend VAD state machine and event emission logic in `src-tauri/src/vad/` and `src-tauri/src/vad_commands.rs`.
- Frontend hook and UI components in `src/hooks/` and `src/components/`.
- Event payload expectations for `vad-state`, `transcript`, and `error` listeners.
- Tests and verification for continuous state transitions, repeated transcription, and session cleanup.
