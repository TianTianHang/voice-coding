# AGENTS.md

Static files served by Vite at stable root-relative URLs and packaged with the frontend.

## Folder Role
- `ten_vad.wasm` is the browser/static VAD artifact if a frontend VAD path is used or documented.
- `tauri.svg` and `vite.svg` are starter/static assets and can be removed only after checking references.

## Editing Notes
- Use `public/` for files that should be fetched by URL, not imported through TypeScript.
- Keep binary artifacts small and documented; native backend VAD libraries belong in `src-tauri/libs/`.
- Update references in `src/` or docs when renaming files.
