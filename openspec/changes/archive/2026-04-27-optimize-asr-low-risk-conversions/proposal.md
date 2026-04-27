## Why

The ASR hot path currently performs avoidable format conversions and buffer copies around already-normalized audio and decoder outputs. These conversions add latency to short voice-command transcription without changing model behavior or user-facing semantics.

## What Changes

- Route VAD-captured 16kHz mono PCM directly into the ASR engine as raw float32 samples instead of encoding it as WAV bytes and decoding it with Symphonia.
- Preserve existing byte-buffer transcription for external `Vec<u8>` audio input, including format detection and validation behavior.
- Optimize greedy token selection so decoder logits can be inspected without copying the full logits tensor into a new owned array.
- Simplify prompt/audio embedding fusion to avoid repeated scans while preserving the exact prompt structure and generated transcription output.
- Add focused tests or checks that prove transcription inputs, stop-token behavior, and stale-session protections remain unchanged.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `stt-qwen3`: VAD integration SHALL pass raw PCM samples to ASR without requiring WAV byte-buffer decoding.
- `audio-preprocessing`: Raw sample input SHALL remain the direct no-decode path for already-normalized 16kHz mono audio.
- `onnx-inference`: Greedy token decoding SHALL avoid unnecessary full-logits buffer copies while preserving argmax semantics.

## Impact

- Affected Rust modules:
  - `src-tauri/src/vad_commands.rs`
  - `src-tauri/stt-qwen3/src/lib.rs`
  - `src-tauri/stt-qwen3/src/decoder.rs`
  - related unit tests under `src-tauri/`
- No frontend API changes.
- No model file, dependency, or ONNX graph changes.
- Validation after implementation should include targeted Rust tests plus `cargo test` for the workspace.
