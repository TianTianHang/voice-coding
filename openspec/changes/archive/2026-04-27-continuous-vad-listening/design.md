## Context

The current implementation uses a backend VAD state machine that moves from `idle` to `listening`, then `recording`, then `processing`, and finally returns to `idle` after transcription. That behavior works for a single utterance, but it creates a UX mismatch for users who expect the app to keep listening after each result. The change affects both the Rust backend session lifecycle and the React frontend state presentation.

## Goals / Non-Goals

**Goals:**
- Keep a single listening session active across multiple utterances.
- Return to `listening` after each transcription completes.
- Preserve `idle` for explicit stop and fatal failure conditions.
- Keep frontend state, transcript, and error handling aligned with backend events.

**Non-Goals:**
- Changing the VAD model, threshold, or audio format.
- Adding partial transcription or streaming ASR results.
- Removing the user control to stop listening.
- Redesigning the entire recorder UI beyond state semantics and copy.

## Decisions

1. **Keep the existing four-state model, but change the terminal transition**
   - Decision: Retain `idle`, `listening`, `recording`, and `processing`, but make `processing -> listening` the normal completion path.
   - Rationale: This minimizes surface area while fixing the UX mismatch.
   - Alternatives considered: Add a new `completed` or `ready` state, but that would complicate frontend and backend contracts without adding meaningful behavior.

2. **Treat `idle` as an explicit stop state only**
   - Decision: `idle` means the user has stopped listening or the backend encountered a session-ending error.
   - Rationale: This removes ambiguity and makes `idle` semantically distinct from “waiting for the next utterance.”
   - Alternatives considered: Reuse `idle` between utterances, but that recreates the confusing UI state we want to eliminate.

3. **Keep one backend session alive until stop**
   - Decision: Maintain the active recorder and session identity across multiple utterances instead of creating a new session for each transcript.
   - Rationale: Continuous listening should preserve transcript accumulation and avoid unnecessary start/stop churn.
   - Alternatives considered: Restart the recorder after every transcript, but that adds latency, more permission churn, and more failure points.

4. **Keep the event contract unchanged, but adjust event timing semantics**
   - Decision: Continue emitting `vad-state`, `transcript`, and `error` events, with `vad-state: listening` emitted after each completed transcription.
   - Rationale: Existing frontend listeners already understand the event model, so this is a low-risk contract evolution.
   - Alternatives considered: Introduce a new event type for session continuity, but the current stream is sufficient.

5. **Use a small copy update to make continuous listening obvious**
   - Decision: Update frontend labels so `listening` reads as “waiting for speech” or similar, while `processing` indicates transcription work.
   - Rationale: The semantic change should be visible to users without requiring them to infer backend behavior.
   - Alternatives considered: Leave the current wording unchanged, but that keeps the ambiguity around whether the session is still active.

## Risks / Trade-offs

- [Risk] Continuous listening may confuse users if the UI does not clearly indicate that the session is still active → [Mitigation] Keep a persistent listening indicator and update copy to emphasize waiting for the next utterance.
- [Risk] Transcription failures could stall the session in `processing` or stop the loop unexpectedly → [Mitigation] On non-fatal transcription errors, emit `error` and return to `listening`; reserve `idle` for explicit stop or fatal device failures.
- [Risk] Keeping one long-lived session may make cleanup bugs more visible → [Mitigation] Add coverage for repeated speech cycles, stop cleanup, and error cleanup.
- [Risk] Frontend session tracking could become stale if an old event arrives late → [Mitigation] Continue using session-scoped event filtering and treat backend events as the source of truth.

## Migration Plan

1. Update the backend state machine so transcription completion transitions back to `listening`.
2. Adjust backend event emission so the frontend receives `vad-state: listening` after each finished utterance.
3. Update the frontend hook and UI copy to reflect continuous listening semantics.
4. Verify repeated utterance cycles, stop behavior, and error recovery with tests.
5. Roll back by restoring `processing -> idle` if the continuous session model exposes regressions in session lifecycle or UI clarity.

## Open Questions

- Should transcription errors always return to `listening`, or should some errors pause the session until the user confirms retry?
- Should the transcript UI visibly separate utterances, or is continuous append sufficient for the first version?
- Should `processing` remain non-interactive, or should the stop button still be available while the backend is transcribing?
