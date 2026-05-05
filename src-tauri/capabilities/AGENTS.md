# AGENTS.md

Tauri capability manifests controlling which frontend windows can call which commands/plugins.

## Folder Role
- `default.json` grants the main window access to app commands and plugin permissions.
- These files are consumed by Tauri at build/runtime; invalid permissions can break command invocation.

## Editing Notes
- Keep permissions minimal and explicit.
- Update this folder when adding commands that require new Tauri/plugin permissions.
- Validate with `nix develop -c pnpm tauri dev` or `nix develop -c pnpm tauri build`.
