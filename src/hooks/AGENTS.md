# AGENTS.md

Custom React hooks that bridge the frontend to Tauri backend commands and event streams. This folder owns session filtering, event merging, and UI-ready state snapshots.

## Module Map
- `useBusinessApi.ts` is the primary product-flow facade for app readiness, voice sessions, transcript handling, Agent turn state, speech output, preferences, and runtime errors.
- `useBackendVAD.ts` is a compatibility/debug hook for legacy VAD lifecycle commands, session ids, transcript/error event filtering, and recording duration.
- `useTranscription.ts` wraps direct debug transcription flows.
- `useAsrStatus.ts` tracks debug ASR preload/readiness state from commands and `asr-status` events.
- `useAgentEvents.ts` manages the legacy ACP content stream for event upserts/appends, confirmations, plans, and session state; primary Agent connection and turn status should come from `useBusinessApi`.

## Backend Links
- Business API hooks depend on `src-tauri/src/business.rs` commands and `app-status-changed`, `voice-session-changed`, `agent-status-changed`, `agent-turn-changed`, `speech-output-changed`, `voice-utterance`, and `runtime-error` events.
- VAD hooks depend on `src-tauri/src/vad_commands.rs` and events emitted from the recorder task.
- ASR hooks depend on `src-tauri/src/asr.rs` runtime snapshots.
- Agent hooks depend on `src-tauri/src/acp/session.rs` commands and `src-tauri/src/acp/events.rs` event mapping.

## Invariants
- Main product UI must use `useBusinessApi` as its source of truth for readiness, voice input, transcript lifecycle, Agent turn state, speech output, preferences, and runtime errors.
- `useBackendVAD`, `useAsrStatus`, direct transcription, and debug TTS status hooks are debug or compatibility entry points; do not reintroduce them into the main assistant console flow.
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
