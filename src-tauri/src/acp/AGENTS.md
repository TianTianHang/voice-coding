# AGENTS.md

Agent Client Protocol integration for connecting this app to an external coding agent process and streaming agent activity back to the UI.

## Module Map
- `mod.rs` re-exports `AcpRuntime` and `AgentEvent`.
- `profile.rs` resolves the configured agent profile/command used to launch or connect.
- `transport.rs` owns process/stdin/stdout or protocol transport details.
- `client.rs` wraps the ACP SDK client calls and message exchange.
- `events.rs` maps raw ACP messages/content/tool updates into frontend `AgentEvent` payloads.
- `session.rs` owns Tauri commands for connect/disconnect/status/prompt/confirmation and runtime state.

## Frontend Links
- `src/hooks/useAgentEvents.ts` consumes `agent-event` and calls all ACP commands.
- `src/components/AgentEventStream.tsx` renders event kinds produced here: `thinking`, `tool`, `result`, `diff`, `confirm`, `error`, and `status`.

## Invariants
- Keep event ids stable enough for frontend de-duplication.
- Preserve `messageId`, `toolCallId`, and `confirmationId` when available; hooks depend on them for merge/upsert and confirmation status.
- Disconnect should terminate child process/session resources and emit a clear status/error event if needed.
- Raw protocol payloads may be included for debugging but should not be required for normal UI rendering.

## Editing Notes
- Avoid blocking reads/writes on the Tauri UI thread; use Tokio process/io utilities.
- Map SDK/protocol errors into user-readable strings at the Tauri command boundary.
- Update OpenSpec specs under `openspec/specs/acp-*` when behavior changes materially.
