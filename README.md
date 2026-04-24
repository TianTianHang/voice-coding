# Voice Coding

A Tauri + React desktop app that provides voice-driven coding through automatic speech recognition. Uses VAD (Voice Activity Detection) to automatically detect when you're speaking and transcribe your voice to text.

## Features

- **VAD-Based Recording**: Automatically detects speech start/stop using the ten-vad WebAssembly engine
- **Auto-Transcription**: Sends recorded audio to Qwen3 ASR backend for speech-to-text
- **Real-time Feedback**: Visual status indicators (Listening, Recording, Processing)
- **Low Resource Usage**: CPU ~2%, Memory ~5MB during active listening

## Requirements

- **Microphone permission**: The app needs access to your microphone via `navigator.mediaDevices.getUserMedia`
- **Tauri v2** with the `stt-qwen3` feature enabled
- Qwen3 ASR model files in the `models/` directory

## Architecture

- `src/hooks/useVAD.ts` - VAD engine wrapper (ten-vad WASM) with state machine (idle → listening → recording → processing)
- `src/hooks/useAudioRecorder.ts` - WAV encoding (PCM, 16kHz, mono, 16-bit)
- `src/hooks/useTranscription.ts` - Tauri invoke wrapper for transcription
- `src/components/` - UI components (VoiceRecorder, AudioVisualizer, TranscriptDisplay, ControlButton)
- `src-tauri/src/asr.rs` - Rust backend: `transcribe` and `transcribe_audio_data` commands

## VAD Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| hop_size | 256 | 16ms @ 16kHz frame size |
| threshold | 0.5 | Speech detection threshold |
| silence_frames | 30 | 480ms of silence triggers stop |

## Troubleshooting

- **"Microphone access denied"**: Grant microphone permission in your system settings
- **"VAD initialization failed"**: Ensure `ten_vad.wasm` is accessible
- **No transcription output**: Verify the Qwen3 model is in `models/` and the `stt-qwen3` feature is enabled

## Development

```bash
pnpm dev          # Start Vite dev server
pnpm build        # Build frontend
pnpm tauri dev    # Start Tauri dev mode
pnpm test         # Run unit tests
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
