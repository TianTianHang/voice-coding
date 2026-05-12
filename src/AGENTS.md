# AGENTS.md

Frontend source for the Tauri desktop UI. This layer renders the voice-first assistant console and talks to Rust through Tauri commands and events.

## Folder Role
- `App.tsx` is intentionally thin and mounts `AssistantConsole`.
- `main.tsx` is the React/Vite entry point.
- `App.css` holds the app-level visual system used by the console components.
- `components/` contains presentational and orchestration components.
- `hooks/` contains Tauri command/event integration and state reducers for backend-driven flows.

## Backend Links
- Commands are called with `invoke` from `@tauri-apps/api/core`; registered command names live in `src-tauri/src/lib.rs`.
- Voice recording debug/compat hooks use `debug_start_listening`, `debug_stop_listening`, `debug_get_vad_state`, `debug_get_vad_config`, and `debug_set_vad_config`; new product flows should use business voice-session commands.
- ASR debug/compat status uses `debug_get_asr_status` and `debug_prepare_asr`; new product flows should prefer `get_app_status`/`prepare_app`.
- Agent console commands use `connect_agent`, `disconnect_agent`, `get_agent_status`, `send_agent_prompt`, and `respond_agent_confirmation` from `src-tauri/src/acp/session.rs`.
- TTS debug controls use `debug_prepare_tts`, `debug_get_tts_status`, `debug_synthesize_tts`, `debug_play_tts`, and `debug_cancel_tts_playback`; new product flows should use speech business commands.

## Event Contracts
- `vad-state` carries `{ state, sessionId }`; treat it as the source of truth for recording state.
- `transcript` carries `{ text, sessionId }`; ignore missing or stale session ids.
- `error` may be a legacy string or `{ message, sessionId }`; prefer session-scoped handling.
- `asr-status`, `tts-state`, and `agent-event` are backend status/event streams consumed by hooks.

## Editing Notes
- Keep component props typed with `interface`; use `type` for object unions/shapes.
- Always clean up Tauri event listeners in `useEffect` returns.
- Catch `invoke` failures and surface user-readable errors instead of throwing from handlers.
- Keep tests next to the hook/component they cover with `*.test.ts` or `*.test.tsx`.

## Validation
- Run `pnpm test` for frontend behavior changes.
- Run `pnpm build` when changing TypeScript types, imports, CSS, or Vite-facing code.
