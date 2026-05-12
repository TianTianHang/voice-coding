# AGENTS.md

Agent Client Protocol integration for connecting this app to an external coding agent process and streaming agent activity back to the UI.

## Module Map
- `mod.rs` re-exports `AcpRuntime` and `AgentEvent`.
- `profile.rs` resolves the configured agent profile/command used to launch or connect.
- `transport.rs` owns process/stdin/stdout or protocol transport details.
- `client.rs` wraps the ACP SDK client calls and message exchange.
- `events.rs` maps raw ACP messages/content/tool updates into frontend `AgentEvent` payloads.
- `timeline.rs` owns the authoritative backend Agent timeline runtime, reducer, snapshot/patch DTOs, and stream confirmation command.
- `session.rs` owns Tauri commands for connect/disconnect/status/prompt/confirmation and runtime state.

## Frontend Links
- `src/hooks/useAgentStream.ts` consumes `get_agent_timeline` and `agent-timeline-changed` as the main UI content stream.
- `src/hooks/useAgentEvents.ts` consumes legacy `agent-event` for debug/compat only.
- `src/components/AgentEventStream.tsx` renders timeline kinds produced here: `thinking`, `message`, `tool`, `diff`, `confirmation`, `error`, `status`, and `fallback`.

## Invariants
- Keep event ids stable enough for frontend de-duplication.
- Preserve `messageId`, `toolCallId`, and `confirmationId` when available; hooks depend on them for merge/upsert and confirmation status.
- Backend timeline patches are authoritative for Agent content merging; the main frontend must not need ACP operation semantics to render correctly.
- Disconnect should terminate child process/session resources and emit a clear status/error event if needed.
- Raw protocol payloads may be included for debugging but should not be required for normal UI rendering.

## Editing Notes
- Avoid blocking reads/writes on the Tauri UI thread; use Tokio process/io utilities.
- Map SDK/protocol errors into user-readable strings at the Tauri command boundary.
- Update OpenSpec specs under `openspec/specs/acp-*` when behavior changes materially.
