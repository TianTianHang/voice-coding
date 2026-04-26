# AGENTS.md

This file contains instructions for agentic coding assistants working in this repository.

## Project Overview

This is a **Tauri v2 + React 19 + TypeScript** desktop application for voice-driven coding using automatic speech recognition (ASR). The project uses:
- **Frontend**: React 19 with TypeScript, Vite build tool
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
```bash
# Development
pnpm tauri dev        # Start Tauri in dev mode

# Build
pnpm tauri build      # Build production app

# Test
cargo test            # Run all Rust tests (workspace)
cargo test <test_name># Run a single test by name
cargo test -p stt-qwen3           # Test specific package
cargo test -p stt-qwen3 --test integration_test  # Run specific test file

# Lint
cargo clippy          # Run Rust linter
cargo clippy --fix    # Auto-fix linting issues
```

### Python Scripts
```bash
# Use the virtual environment
source .venv/bin/activate  # or: .venv/bin/python
python scripts/verify_onnx_inputs.py
```

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
- `STT_MODEL_DIR` - Path to model files (default: `./models`)
- `ORT_DYLIB_PATH` - Path to ONNX Runtime library (handled by Nix setup)

## File Locations
- Test audio: `test_audio/`
- Models: `models/onnx_models/`
- Documentation: `docs/`
- Build output: `dist/` (frontend), `src-tauri/target/` (Rust)
