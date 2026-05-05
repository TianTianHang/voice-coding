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
- Voice recording uses `start_listening`, `stop_listening`, `get_vad_state`, `get_vad_config`, and `set_vad_config` from `src-tauri/src/vad_commands.rs`.
- ASR status uses `get_asr_status` and `prepare_asr` from `src-tauri/src/asr.rs`.
- Agent console commands use `connect_agent`, `disconnect_agent`, `get_agent_status`, `send_agent_prompt`, and `respond_agent_confirmation` from `src-tauri/src/acp/session.rs`.
- TTS controls use `prepare_tts`, `get_tts_status`, `synthesize_tts`, `play_tts`, and `cancel_tts_playback` from `src-tauri/src/tts.rs`.

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
