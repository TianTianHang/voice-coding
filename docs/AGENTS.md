# AGENTS.md

Project documentation for model inputs, TEN VAD setup, virtual audio, Tauri permissions, and future VAD work.

## Document Map
- `model_inputs_report.md` and `model_inputs_spec.json` describe ONNX model input expectations.
- `TEN_VAD_*` documents explain native VAD build/download/setup and platform notes.
- `VIRTUAL_AUDIO_GUIDE.md` describes virtual audio devices for testing voice flows.
- `tauri-permissions.md` documents permission/capability behavior.
- `vad-future-enhancements.md` captures follow-up ideas for VAD improvements.

## Relationships
- Update docs when changing scripts, model asset requirements, Tauri capabilities, VAD thresholds, or audio pipeline assumptions.
- `scripts/verify_onnx_inputs.py` may generate or validate model input artifacts here.
- OpenSpec requirements live in `openspec/`; docs here are practical usage/setup references.

## Editing Notes
- Keep commands consistent with root `AGENTS.md`.
- Prefer current paths and environment variables over historical names.
- Make setup docs explicit about OS-specific prerequisites.
