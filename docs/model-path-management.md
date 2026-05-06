# Model Path Management

Voice Coding resolves local ASR and TTS assets through a shared model root contract. The recommended model root is `VOICE_CODING_MODEL_HOME`; when it is not set, development defaults use `models` in the repository.

## Standard Layout

```text
<model-home>/
├── asr/
│   └── qwen3-asr-0.6b-onnx/
│       ├── tokenizer.json
│       ├── config.json
│       ├── embed_tokens.bin
│       └── onnx_models/
│           ├── encoder.int4.onnx or encoder.onnx
│           ├── decoder_init.int4.onnx or decoder_init.onnx
│           ├── decoder_step.int4.onnx or decoder_step.onnx
│           └── decoder_weights.int4.data
└── tts/
    └── moss-tts-nano-100m-onnx/
        ├── MOSS-TTS-Nano-100M-ONNX/
        │   ├── browser_poc_manifest.json
        │   ├── tts_browser_onnx_meta.json
        │   ├── tokenizer.model
        │   └── MOSS TTS ONNX and data files
        └── MOSS-Audio-Tokenizer-Nano-ONNX/
            ├── codec_browser_onnx_meta.json
            └── MOSS codec ONNX and data files
```

## Resolution Priority

ASR uses this priority:

1. `STT_MODEL_DIR`, interpreted as the direct Qwen3 ASR model directory.
2. `VOICE_CODING_MODEL_HOME`, resolved to `<model-home>/asr/qwen3-asr-0.6b-onnx`.
3. Tauri app data directory, resolved to `<app-data>/models/asr/qwen3-asr-0.6b-onnx` when assets exist.
4. Repository development fallback, resolved to `models/asr/qwen3-asr-0.6b-onnx`.
5. Legacy development fallback, resolved to `models` when `models/tokenizer.json` or `models/onnx_models` exists.

TTS uses this priority:

1. `MOSS_TTS_MODEL_DIR`, interpreted as the direct `MOSS-TTS-Nano-100M-ONNX` component directory.
2. `VOICE_CODING_MODEL_HOME`, resolved to `<model-home>/tts/moss-tts-nano-100m-onnx`.
3. Tauri app data directory, resolved to `<app-data>/models/tts/moss-tts-nano-100m-onnx` when assets exist.
4. Repository development fallback, resolved to `models/tts/moss-tts-nano-100m-onnx`.
5. Legacy development fallback, resolved to `models/moss-tts` when `models/moss-tts/MOSS-TTS-Nano-100M-ONNX/browser_poc_manifest.json` exists.

## Download Scripts

Use the scripts without arguments for the standard layout:

```bash
scripts/download_model.sh
scripts/download_moss_tts_models.sh
```

The default targets are `${VOICE_CODING_MODEL_HOME:-models}/asr/qwen3-asr-0.6b-onnx` and `${VOICE_CODING_MODEL_HOME:-models}/tts/moss-tts-nano-100m-onnx`. Both scripts still accept an explicit first argument and use it unchanged.

## Diagnostics

Backend ASR and TTS status snapshots include a structured `model` object with `kind`, `modelId`, `engineName`, `packageDir`, `modelDir`, `source`, `legacyLayout`, `missingFiles`, and optional `error`. ASR also keeps the top-level `modelDir` field for compatibility.
