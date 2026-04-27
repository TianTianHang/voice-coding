## 1. VAD Sample Path

- [x] 1.1 Add a small conversion helper for mono 16kHz `i16` PCM to normalized `Vec<f32>` samples using `sample as f32 / 32768.0`.
- [x] 1.2 Change backend VAD transcription to call the ASR engine with `AudioInput::Samples(samples, SAMPLE_RATE)` instead of WAV bytes.
- [x] 1.3 Remove the real-time VAD path's WAV encode/decode dependency while keeping generic `transcribe_audio_data(Vec<u8>)` behavior unchanged.

## 2. Decoder Copy Reduction

- [x] 2.1 Refactor greedy token selection to scan extracted logits tensor data directly without constructing an owned `Array3<f32>`.
- [x] 2.2 Preserve decoder shape validation and stop-token behavior for `IM_END_ID`, `ENDOFTEXT_ID`, and `max_new_tokens`.
- [x] 2.3 Simplify `embed_and_fuse` to map audio pad tokens with a monotonic audio index while preserving mismatch errors.

## 3. Tests

- [x] 3.1 Add unit coverage for the VAD `i16` to `f32` conversion helper, including positive, negative, and zero samples.
- [x] 3.2 Add or update tests proving backend VAD transcription constructs `AudioInput::Samples` for raw PCM input.
- [x] 3.3 Add decoder tests for direct logits argmax over the last sequence position.
- [x] 3.4 Add embedding fusion tests confirming audio pad count mismatch and successful ordered fusion still behave as before.

## 4. Verification

- [x] 4.1 Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] 4.2 Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`.
- [x] 4.3 Record any unavailable model-dependent checks or environmental blockers in the implementation summary.
