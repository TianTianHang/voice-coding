## Context

The backend VAD flow currently spans cpal audio capture, a VAD state machine, a background transcription task, and React listeners in the frontend. That split is functional, but the lifecycle boundaries are too loose: stop does not clearly cancel in-flight work, temporary audio files can survive failures, and the frontend can mix results from different sessions.

The change needs to improve reliability without changing the core user flow. The app should still behave as a simple start/stop recorder with automatic transcription, but the boundaries around session ownership, cleanup, and state reporting need to become explicit.

## Goals / Non-Goals

**Goals:**
- Make session start/stop deterministic.
- Prevent stale transcription results from appearing after a session ends.
- Guarantee cleanup of temporary audio artifacts.
- Keep frontend state and transcript display aligned with backend events.
- Reduce ambiguity around which layer owns state transitions.

**Non-Goals:**
- Reworking the core VAD algorithm.
- Adding live streaming transcription.
- Adding multi-device microphone selection.
- Redesigning the UI beyond stability-related state handling.

## Decisions

### 1. Introduce explicit recording-session ownership

Each listening cycle should have an explicit session identity owned by the backend. Events emitted by the backend should carry that identity so the frontend can ignore stale results.

Alternatives considered:
- Relying on timing alone: simpler, but vulnerable to late events.
- A shared global boolean: easier to wire, but cannot distinguish old results from active ones.

### 2. Treat stop as cancellation, not just a state toggle

Stopping should cancel or invalidate the active session before any pending transcription result can be applied. The backend should remain authoritative about when the session is idle again.

Alternatives considered:
- Optimistic frontend idle state: feels fast, but risks UI/backend divergence.
- Waiting only for audio stream teardown: insufficient because ASR may still be running.

### 3. Make cleanup unconditional in transcription code paths

Temporary audio artifacts should be removed in a finally-style cleanup path so failures do not accumulate files.

Alternatives considered:
- Best-effort cleanup after success only: simpler, but leaks on errors.
- Periodic cleanup jobs: useful as a fallback, but not a replacement for deterministic cleanup.

### 4. Keep the frontend session-scoped and event-driven

The frontend should not maintain its own independent notion of the current backend session state. It should subscribe to backend events and use the session id to decide whether to render or ignore transcripts.

Alternatives considered:
- Local transcript history ref only: too easy to desynchronize.
- Polling `get_vad_state`: adds latency and still does not solve stale transcript ordering.

## Risks / Trade-offs

- Late transcript events may still be produced by in-flight work → mitigate by tagging events with session identity and ignoring stale results in the frontend.
- More state plumbing increases code complexity → mitigate by centralizing ownership in the backend recorder state and keeping the frontend thin.
- Cancellation may interrupt some legitimate transcriptions near stop boundaries → mitigate by treating stop as a hard session boundary and documenting that behavior.
- Cleanup logic must be careful not to delete active files too early → mitigate by only cleaning up after transcription completes or aborts.
