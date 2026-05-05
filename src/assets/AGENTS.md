# AGENTS.md

Static frontend assets imported by React/Vite code.

## Folder Role
- Use this for assets that are bundled through Vite imports from `src/`.
- Public files that must keep stable URL paths belong in `public/`, not here.

## Editing Notes
- Keep filenames descriptive and lowercase unless an existing imported asset requires a specific name.
- Update importing components when replacing or moving assets.
- Avoid committing large generated binaries unless they are required for the app to run.
