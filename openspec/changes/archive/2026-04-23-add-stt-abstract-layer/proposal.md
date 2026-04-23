# Proposal: Add STT Abstract Layer

## Why

The application needs Speech-to-Text (STT) capabilities to enable voice-based coding features. Currently, there is no STT implementation. An abstract layer is needed to support multiple STT engines (local models, cloud APIs) with a unified API, making the system extensible and maintainable. Starting with Qwen3-ASR-0.6B ONNX for CPU-only local inference provides a privacy-preserving, cost-effective solution with support for 30 languages.

## What Changes

- **New STT Core Crate** (`stt-core`): Define abstract traits (`SttEngine`) and common types (config, results, errors)
- **Qwen3 ASR Implementation** (`stt-qwen3`): Complete implementation of Qwen3-ASR-0.6B ONNX model with:
  - Audio preprocessing (16kHz resampling, Mel spectrogram computation)
  - ONNX Runtime inference (encoder + decoder with KV cache)
  - Tokenization and prompt building
  - VAD-based long audio chunking
- **Tauri Integration**: Add `transcribe` command using the STT abstract layer
- **Feature Flags**: Compile-time engine selection for zero-overhead abstraction
- **Workspace Structure**: Convert to Cargo workspace with modular crates

## Capabilities

### New Capabilities
- `stt-engine`: Core STT abstraction defining the interface for all speech-to-text engines
- `stt-qwen3`: Qwen3-ASR-0.6B ONNX implementation with full inference pipeline
- `audio-preprocessing`: Audio loading, resampling, and Mel spectrogram computation
- `onnx-inference`: ONNX Runtime integration for model inference with KV cache management

### Modified Capabilities
*None - this is a new feature addition*

## Impact

### Code Structure
- **New Workspace Members**: `stt-core/`, `stt-qwen3/`
- **Modified Files**: `src-tauri/Cargo.toml` (workspace config), `src-tauri/src/lib.rs` (add STT command)
- **Build Configuration**: Add feature flags for engine selection (`stt-qwen3`, future `stt-whisper`, etc.)

### Dependencies
- **Rust Crates**: `ort` (ONNX Runtime), `symphonia` (audio), `rustfft` (FFT), `tokenizers`, `ndarray`, `async-trait`
- **Model Files**: Download ~2.5GB of ONNX models and tokenizer from HuggingFace
- **System Libraries**: ONNX Runtime via Nix (already available in nixpkgs)

### API Surface
- **Tauri Commands**: New `transcribe(audio_path, language?) -> Result<String>` command
- **Public API**: `stt_core::SttEngine` trait for future engine implementations
- **Configuration**: Compile-time feature flags + runtime `SttConfig` struct

### Performance Characteristics
- **Memory**: ~5-6GB peak with INT8 models (reduced with VAD chunking)
- **Speed**: RTF 0.32x on desktop (3x realtime), RTF 0.71x with VAD on Intel N100
- **CPU**: CPU-only inference, no GPU required
- **Language Support**: 30 languages out of the box

### Future Extensibility
- Easy to add new STT engines (Whisper, Azure, Google, etc.) by implementing `SttEngine` trait
- Streaming STT support through optional `StreamingStt` trait
- Batch processing through optional `BatchStt` trait
