## 1. Backend state machine

- [x] 1.1 Change the VAD state machine to return to `listening` after successful transcription or recoverable transcription failure.
- [x] 1.2 Keep `idle` reserved for explicit stop and fatal device/session failure paths.
- [x] 1.3 Update state emission paths so `vad-state` reflects the continuous listening loop.

## 2. Frontend state handling

- [x] 2.1 Update the frontend VAD hook to treat `listening` as the active waiting state between utterances.
- [x] 2.2 Preserve the active session across multiple transcription cycles without forcing a new start after each transcript.
- [x] 2.3 Adjust button and status copy so users understand the session is still active.

## 3. Transcript and error flow

- [x] 3.1 Confirm transcript events append across multiple utterances in the same session.
- [x] 3.2 Keep recoverable transcription errors visible while remaining in the listening loop.
- [x] 3.3 Ensure fatal device errors still terminate the session and transition to `idle`.

## 4. Verification

- [x] 4.1 Add or update Rust tests for repeated utterance cycles and the `processing -> listening` transition.
- [x] 4.2 Add or update frontend tests for status presentation and session persistence behavior.
- [x] 4.3 Run `cargo test`, `cargo clippy`, `pnpm test`, `pnpm build`, and `pnpm tauri build` and record any blockers if a check fails.

Blockers recorded for 4.3:
- `cargo clippy` failed in this environment due to missing linker wrapper path (`/nix/store/.../ld-wrapper.sh`).
- `pnpm tauri build` completed compilation and package generation but failed at final bundling because `/usr/bin/xdg-open` is missing.
