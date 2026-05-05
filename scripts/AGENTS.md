# AGENTS.md

Utility scripts for model setup, TEN VAD native library setup, virtual audio test devices, and model input verification.

## Script Map
- `download_model.sh` downloads ASR model assets.
- `download_moss_tts_models.sh` downloads MOSS TTS model assets.
- `build_ten_vad.sh` builds the TEN VAD native library.
- `download_ten_vad.sh` downloads prebuilt TEN VAD binaries.
- `setup_virtual_audio.sh` and `cleanup_virtual_audio.sh` manage virtual audio devices for testing.
- `verify_onnx_inputs.py` inspects ONNX model input metadata and writes/validates docs artifacts.

## Relationships
- Package scripts `vad:build`, `vad:download`, and `vad:setup` call TEN VAD scripts.
- Backend VAD library discovery expects files under `src-tauri/libs/<platform>/<arch>/`.
- Python scripts should run with `.venv/bin/python` and keep reports aligned with `docs/model_inputs_*`.

## Editing Notes
- Use portable shell where possible; clearly guard Linux-only virtual audio operations.
- Do not embed credentials or private model URLs.
- Prefer explicit error messages and non-zero exits when prerequisites are missing.
