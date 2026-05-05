# AGENTS.md

React UI components for the voice assistant console. Components here should stay mostly declarative and delegate backend/event state to hooks in `src/hooks/`.

## Module Map
- `AssistantConsole.tsx` is the main experience shell: voice state, agent timeline, VAD threshold controls, close behavior, and TTS controls.
- `AgentEventStream.tsx` renders ACP agent events, tool calls, confirmations, diffs, and session state updates.
- `VoiceRecorder.tsx` is the focused recorder UI around backend VAD state.
- `AudioVisualizer.tsx` renders visual feedback for listening/recording states.
- `ControlButton.tsx` is the reusable start/stop/action button.
- `TranscriptDisplay.tsx` presents transcript text and errors.

## Key Relationships
- `AssistantConsole` consumes `useBackendVAD`, `useAsrStatus`, and `useAgentEvents`.
- Agent confirmation buttons must call `respondToConfirmation` from `useAgentEvents`; do not call Tauri directly from deep rendering helpers unless the hook cannot own it.
- VAD state names must match `src/hooks/useBackendVAD.ts` and backend `VadState` serialization.

## Editing Notes
- Preserve existing visual language in `App.css`; avoid introducing unrelated design systems.
- Keep pure derivation helpers exported when tests cover them, such as voice state derivation and timeline expansion decisions.
- Prefer small typed helper functions over inline complex conditionals in JSX.
- Do not store duplicated backend source-of-truth state unless it is UI-only presentation state.

## Validation
- Run `pnpm test src/components` for component or derivation changes.
- Run `pnpm build` after changing JSX structure, exported types, or CSS class usage.
