# AGENTS.md

Tests for the Qwen3 ASR crate.

## Module Map
- `engine_test.rs` covers engine-level behavior.
- `integration_test.rs` covers model-backed transcription paths when assets are available.
- `boundary_test.rs` covers edge cases and boundary conditions.
- `common/` contains fixtures and mock sessions shared by tests.

## Editing Notes
- Prefer mocks for shape/control-flow tests that do not require real model files.
- Gate or skip model-backed tests gracefully when large model assets are unavailable.
- Run `nix develop -c cargo test -p stt-qwen3` after changes.
