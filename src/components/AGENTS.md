# AGENTS.md

React UI components for the voice assistant console. Components here should stay mostly declarative and delegate backend/event state to hooks in `src/hooks/`.

## Module Map
- `AssistantConsole.tsx` is the main experience shell: business API voice state, transcript actions, Agent timeline, close behavior, and speech controls.
- `AgentEventStream.tsx` renders backend Agent timeline items, including tool calls, confirmations, diffs, messages, thinking, errors, and fallback/status updates.
- `VoiceRecorder.tsx` is the focused recorder UI around backend VAD state.
- `AudioVisualizer.tsx` renders visual feedback for listening/recording states.
- `ControlButton.tsx` is the reusable start/stop/action button.
- `TranscriptDisplay.tsx` presents transcript text and errors.

## Key Relationships
- `AssistantConsole` consumes `useBusinessApi` as the main product-flow facade for app readiness, voice sessions, transcript lifecycle, Agent connection/turn status, speech output, and runtime errors.
- `AssistantConsole` consumes `useAgentStream` for Agent content-stream rendering: thinking, tool calls, result text, diffs, confirmations, plans, and confirmation responses.
- `AssistantConsole` must not import `useAgentEvents`; that hook is reserved for debug/compat flows.
- Do not reintroduce `useBackendVAD`, `useAsrStatus`, direct debug transcription, or debug TTS status hooks into the main console flow; those belong to debug/compat views such as `DebugToolsWindow` or focused legacy components.
- Agent confirmation buttons must call `respondToConfirmation` from `useAgentStream`; do not call Tauri directly from deep rendering helpers unless the hook cannot own it.
- Voice display state in the main console should be derived from business `VoiceSessionStatus`, not legacy VAD state names.

## Editing Notes
- Preserve existing visual language in `App.css`; avoid introducing unrelated design systems.
- Keep pure derivation helpers exported when tests cover them, such as voice state derivation and timeline expansion decisions.
- Prefer small typed helper functions over inline complex conditionals in JSX.
- Do not store duplicated backend source-of-truth state unless it is UI-only presentation state.

## Validation
- Run `pnpm test src/components` for component or derivation changes.
- Run `pnpm build` after changing JSX structure, exported types, or CSS class usage.
