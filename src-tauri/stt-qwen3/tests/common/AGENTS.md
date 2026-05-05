# AGENTS.md

Shared test support for `stt-qwen3`.

## Folder Role
- `fixtures.rs` provides reusable fixture data and paths.
- `mock_sessions.rs` provides session-manager mocks for encoder/decoder/prompt tests.
- `mod.rs` exposes shared helpers to integration and boundary tests.

## Editing Notes
- Keep fixtures small and deterministic.
- Do not make common helpers depend on local machine model paths unless the calling test can skip when unavailable.
