# AGENTS.md

Custom React hooks that bridge the frontend to Tauri backend commands and event streams. This folder owns subscriptions, command facades, and UI-ready state snapshots.

## Module Map
- `useBusinessApi.ts` is the primary product-flow facade for app readiness, voice sessions, transcript handling, Agent turn state, speech output, preferences, and runtime errors.
- `useBackendVAD.ts` is a compatibility/debug hook for legacy VAD lifecycle commands, session ids, transcript/error event filtering, and recording duration.
- `useTranscription.ts` wraps direct debug transcription flows.
- `useAsrStatus.ts` tracks debug ASR preload/readiness state from commands and `asr-status` events.
- `useAgentStream.ts` is the primary Agent content-stream facade. It consumes backend `AgentTimelineSnapshot`/`AgentTimelinePatch` from `get_agent_timeline` and `agent-timeline-changed`, and only performs mechanical reset/upsert/replace updates.
- `useAgentEvents.ts` manages the legacy ACP content stream for debug/compat views only; do not use it as the main console content source.

## Backend Links
- Business API hooks depend on `src-tauri/src/business.rs` commands and `app-status-changed`, `voice-session-changed`, `agent-status-changed`, `agent-turn-changed`, `speech-output-changed`, `voice-utterance`, and `runtime-error` events.
- VAD hooks depend on `src-tauri/src/vad_commands.rs` and events emitted from the recorder task.
- ASR hooks depend on `src-tauri/src/asr.rs` runtime snapshots.
- Agent stream hooks depend on `src-tauri/src/acp/timeline.rs` and the backend runtime that publishes UI-ready timeline patches.

## Invariants
- Main product UI must use `useBusinessApi` as its source of truth for readiness, voice input, transcript lifecycle, Agent turn state, speech output, preferences, and runtime errors.
- Main product UI must use `useAgentStream` as its source of truth for Agent thinking/message/tool/diff/confirmation/error timeline content.
- `useBackendVAD`, `useAsrStatus`, direct transcription, and debug TTS status hooks are debug or compatibility entry points; do not reintroduce them into the main assistant console flow.
- Backend session ids are authoritative; stale `transcript`, `error`, or `vad-state` payloads must not mutate current UI state.
- `debug_stop_listening` invalidates the backend session; frontend should wait for backend `vad-state` rather than forcing idle.
- Event listeners must be disposed and guarded against updates after unmount.
- `useAgentStream` must not interpret ACP operations; backend timeline item ids and patches are authoritative for merging.

## Editing Notes
- Export pure helpers when the behavior is subtle and should be unit tested.
- Keep hook return interfaces explicit and stable for components.
- Catch command failures inside public hook actions and convert them into stateful errors or labels.

## Validation
- Run `pnpm test src/hooks` after hook changes.
- Run related component tests if changing returned shapes or event semantics.
