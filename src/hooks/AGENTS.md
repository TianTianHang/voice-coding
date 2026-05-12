# AGENTS.md

Custom React hooks that bridge the frontend to Tauri backend commands and event streams. This folder owns session filtering, event merging, and UI-ready state snapshots.

## Module Map
- `useBackendVAD.ts` is a compatibility/debug hook for legacy VAD lifecycle commands, session ids, transcript/error event filtering, and recording duration.
- `useTranscription.ts` wraps direct debug transcription flows.
- `useAsrStatus.ts` tracks debug ASR preload/readiness state from commands and `asr-status` events.
- `useAgentEvents.ts` manages ACP connection state, event upserts/appends, confirmations, plans, and session state.

## Backend Links
- VAD hooks depend on `src-tauri/src/vad_commands.rs` and events emitted from the recorder task.
- ASR hooks depend on `src-tauri/src/asr.rs` runtime snapshots.
- Agent hooks depend on `src-tauri/src/acp/session.rs` commands and `src-tauri/src/acp/events.rs` event mapping.

## Invariants
- Backend session ids are authoritative; stale `transcript`, `error`, or `vad-state` payloads must not mutate current UI state.
- `debug_stop_listening` invalidates the backend session; frontend should wait for backend `vad-state` rather than forcing idle.
- Event listeners must be disposed and guarded against updates after unmount.
- Agent tool events are upserted by tool call id; message events may append into existing message ids.

## Editing Notes
- Export pure helpers when the behavior is subtle and should be unit tested.
- Keep hook return interfaces explicit and stable for components.
- Catch command failures inside public hook actions and convert them into stateful errors or labels.

## Validation
- Run `pnpm test src/hooks` after hook changes.
- Run related component tests if changing returned shapes or event semantics.
