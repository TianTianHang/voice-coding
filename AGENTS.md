# AGENTS.md

This file contains instructions for agentic coding assistants working in this repository.

## Project Overview

This is a **Tauri v2 + React 19 + TypeScript** desktop application for voice-driven coding using automatic speech recognition (ASR). The project uses:
- **Frontend**: React 19 with TypeScript, Vite build tool, Tailwind CSS
- **Backend**: Rust (Tauri) with Qwen3 ASR engine
- **Python**: Utility scripts in `scripts/` using the `.venv` virtual environment

## Build/Test Commands

### Frontend (TypeScript/React)
```bash
# Development
pnpm dev              # Start Vite dev server (port 1420)

# Build
pnpm build            # Build frontend (runs tsc + vite build)

# Test
pnpm test             # Run all vitest tests
pnpm test <pattern>   # Run tests matching a pattern (single test)
```

### Backend (Rust/Tauri)
> IMPORTANT: Run Rust commands inside the Nix dev shell to ensure linker/toolchain consistency.
> Prefer `nix develop -c <command>` for all Cargo operations.

```bash
# Development
nix develop -c pnpm tauri dev        # Start Tauri in dev mode

# Build
nix develop -c pnpm tauri build      # Build production app

# Test
nix develop -c cargo test            # Run all Rust tests (workspace)
nix develop -c cargo test <test_name># Run a single test by name
nix develop -c cargo test -p stt-qwen3           # Test specific package
nix develop -c cargo test -p stt-qwen3 --test integration_test  # Run specific test file

# Lint
nix develop -c cargo clippy          # Run Rust linter
nix develop -c cargo clippy --fix    # Auto-fix linting issues
```

### Python Scripts
```bash
# Use the virtual environment
source .venv/bin/activate  # or: .venv/bin/python
python scripts/verify_onnx_inputs.py
```

## OpenSpec Workflow

This repository uses OpenSpec for non-trivial product or architecture changes. Agents should treat OpenSpec as the default planning and execution loop whenever a request affects behavior, public contracts, multi-file implementation, or architecture.

### Workflow Modes
- **Explore**: Use the `openspec-explore` skill when the user wants to discuss, compare options, investigate feasibility, or clarify requirements without implementing yet. Explore mode may read code and OpenSpec artifacts, but must not edit implementation files.
- **Propose**: Use the `openspec-propose` skill when a new change needs a formal proposal. Create a change under `openspec/changes/<change-name>/` with `proposal.md`, `design.md`, `specs/**/*.md`, and `tasks.md` as required by the schema.
- **Apply**: Use the `openspec-apply-change` skill when the user asks to implement an active change, continue work on a change, or when an approved proposal has complete artifacts and no further product decisions are needed.
- **Archive**: Use the `openspec-archive-change` skill when implementation is complete, required checks have passed or documented blockers are accepted, and the specs should be promoted into `openspec/specs/`.

### Autonomous Rotation
Agents may rotate through OpenSpec phases without asking for confirmation when the user's intent is clear and the next phase is mechanically implied:
- If the user asks for a proposal or design, create or update the OpenSpec change and run `openspec validate <change> --strict`.
- If the user asks to implement an active change and `tasks.md` is complete, proceed through the tasks in order, updating checkboxes as work completes.
- If a task reveals missing or incorrect requirements, pause implementation only long enough to update the relevant OpenSpec artifact, validate, then continue.
- If all tasks are complete and validation/checks pass, offer to archive; archive directly only when the user has asked for autonomous completion or explicitly requested archiving.

Do not create a new OpenSpec change for small, local bug fixes or mechanical edits unless the user requests it or the change modifies documented behavior.

### Required Commands
```bash
openspec list --json
openspec status --change <change-name> --json
openspec validate <change-name> --strict
```

During apply work, also run the checks listed in that change's `tasks.md`. For Rust commands, prefer the Nix shell form from this file, for example:
```bash
nix develop -c cargo test -p <crate>
nix develop -c cargo clippy -p <crate> --all-targets
```

### Artifact Rules
- Keep OpenSpec artifacts in Simplified Chinese unless an existing artifact uses another language.
- Specs define **what** the system must do; design explains **how**; tasks track implementation and verification.
- Delta specs must use `## ADDED Requirements`, `## MODIFIED Requirements`, `## REMOVED Requirements`, or `## RENAMED Requirements`.
- Every requirement needs at least one `#### Scenario:` with clear `WHEN` / `THEN` statements.
- For `MODIFIED Requirements`, copy the full existing requirement block and edit it; do not provide a partial delta.
- Keep `tasks.md` checkboxes in `- [ ]` / `- [x]` format so apply automation can track progress.

## Code Style Guidelines

### TypeScript/React (src/)
- **Imports**: Keep imports organized - external libs first, then internal modules
  ```typescript
  import { useState, useEffect } from "react";
  import { invoke } from "@tauri-apps/api/core";
  import { MyComponent } from "../components/MyComponent";
  ```
- **Type definitions**: Use `type` for object shapes, `interface` for component props
- **Components**: Use function components with hooks. No class components.
- **File naming**: `PascalCase.tsx` for components, `camelCase.ts` for utilities/hooks
- **Hooks**: Custom hooks use `use` prefix and return typed interfaces
  ```typescript
  export function useMyHook(): MyHookResult { ... }
  ```
- **Error handling**: Always catch and handle errors from Tauri invokes
  ```typescript
  try {
    await invoke("command_name");
  } catch (e) {
    setError(String(e));
  }
  ```
- **State management**: Use React hooks (useState, useRef, useCallback)
- **Event listeners**: Clean up listeners in useEffect return function
- **Types**: Export types used by other modules. Keep types adjacent to their usage.

### Rust (src-tauri/)
- **Crates**: Workspace with `voice-coding` (main), `stt-core`, and `stt-qwen3`
- **Error handling**: Use `thiserror` for error enums, convert errors with `.map_err(|e| e.to_string())?`
- **Async**: Use `#[tokio::test]` for async tests. Commands use `async fn` when needed.
- **Concurrency**: Use `parking_lot::Mutex` for locking, `crossbeam-channel` for channels
- **Tauri commands**: Mark with `#[tauri::command]`, return `Result<T, String>`
- **Module structure**: Organize by feature (audio/, vad/, asr.rs). Re-export commonly used items.
- **Testing**: Unit tests go in same file with `#[cfg(test)]`. Integration tests in `tests/` directory.
- **Feature flags**: Use `#[cfg(feature = "stt-qwen3")]` for optional code
- **Naming**: 
  - Functions: `snake_case`
  - Types/Enums: `PascalCase`
  - Constants: `SCREAMING_SNAKE_CASE`
- **Dependencies**: Use workspace dependencies defined in root `Cargo.toml`

### Python (scripts/)
- **Style**: Follow PEP 8 conventions
- **Type hints**: Use type hints for function parameters and returns
  ```python
  def analyze_model(model_path: Path) -> dict | None:
  ```
- **Docstrings**: Use triple-quoted docstrings for modules and functions
- **Virtual env**: Always use `.venv/bin/python` to run scripts

## Architecture Notes

### Frontend Architecture
- `src/App.tsx` - Main entry
- `src/hooks/` - Custom React hooks (useBackendVAD, useTranscription)
- `src/components/` - UI components (VoiceRecorder, AudioVisualizer, TranscriptDisplay, ControlButton)
- Tauri invoke calls communicate with Rust backend via `@tauri-apps/api/core`

### Backend Architecture
- `src-tauri/src/lib.rs` - Tauri app setup, command registration
- `src-tauri/src/asr.rs` - ASR transcription commands
- `src-tauri/src/vad_commands.rs` - VAD recording commands
- `src-tauri/src/audio/` - Audio recording module
- `src-tauri/src/vad/` - Voice Activity Detection (VAD) engine
- `src-tauri/stt-core/` - Core ASR traits and types
- `src-tauri/stt-qwen3/` - Qwen3 ASR implementation

### Key Events (Frontend ← Backend)
- `vad-state` - VAD state updates with `{ state, sessionId }`; frontend should treat backend events as source of truth
- `transcript` - Session-scoped transcript event `{ text, sessionId }`; ignore stale session events
- `error` - Session-scoped error event `{ message, sessionId }` for unified UI error handling

### Session Ownership and Cleanup
- Backend owns recording session identity and lifecycle boundaries.
- `stop_listening` invalidates the active session before returning so late results can be dropped safely.
- Frontend should not force idle state locally; it should wait for backend `vad-state` events.
- Temporary audio files in `transcribe_audio_data` must be cleaned up on both success and failure paths.

## Environment Variables
- `VOICE_CODING_MODEL_HOME` - Recommended root for local model assets (default development root: `./models`)
- `STT_MODEL_DIR` - Compatibility override pointing directly to the Qwen3 ASR model directory
- `MOSS_TTS_MODEL_DIR` - Compatibility override pointing directly to the `MOSS-TTS-Nano-100M-ONNX` component directory
- `ORT_DYLIB_PATH` - Path to ONNX Runtime library (handled by Nix setup)

## File Locations
- Test audio: `test_audio/`
- STT ONNX models: `models/asr/qwen3-asr-0.6b-onnx/onnx_models/` under the model home, with legacy `models/onnx_models/` fallback
- MOSS TTS models: `models/tts/moss-tts-nano-100m-onnx/`, with legacy `models/moss-tts/` fallback
- Documentation: `docs/`
- Build output: `dist/` (frontend), `src-tauri/target/` (Rust)
