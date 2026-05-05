# AGENTS.md

Property-test regression seeds for the Qwen3 ASR crate.

## Folder Role
- Files here are generated or maintained by property-based tests to reproduce previously found failing inputs.
- They are test artifacts, not runtime assets.

## Editing Notes
- Do not delete regression files unless the related test behavior was intentionally removed.
- Keep files deterministic and small so CI/test runs can replay failures quickly.
