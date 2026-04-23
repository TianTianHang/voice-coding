# Tasks: STT Test Implementation

Implementation checklist for comprehensive STT inference testing.

## 1. Test Infrastructure Setup

- [x] 1.1 Add `proptest` and `mockall` to dev dependencies in `stt-qwen3/Cargo.toml`
- [x] 1.2 Create `stt-qwen3/tests/common/` directory structure
- [x] 1.3 Create `tests/common/mod.rs` with test helper module exports
- [x] 1.4 Design and create `SessionManager` trait in appropriate location (`stt-core` or new crate)
- [x] 1.5 Implement `MockSessionManager` in `tests/common/mock_sessions.rs`
- [x] 1.6 Create mock helper functions for common test scenarios (deterministic outputs, error injection)
- [x] 1.7 Add test data generators to `tests/common/fixtures.rs` (mel spectrograms, audio samples, token sequences)
- [x] 1.8 Refactor production code to accept `SessionManager` trait where needed (minimal invasive changes)
- [x] 1.9 Verify cargo test runs successfully with new infrastructure (no actual tests yet)

## 2. Decoder Module Tests

### 2.1 Embed and Fuse Tests

- [x] 2.1.1 Add test for successful audio-text embedding fusion with correct pad positions
- [x] 2.1.2 Add test for error when audio pad count doesn't match encoder output count
- [x] 2.1.3 Add test for embedding fusion with all text tokens (no audio)
- [x] 2.1.4 Add test for embedding fusion with all audio tokens (no text)
- [x] 2.1.5 Add edge case test for single token fusion
- [x] 2.1.6 Add edge case test for very long sequences

### 2.2 Decoder Init Tests

- [x] 2.2.1 Add test for successful decoder initialization returning first token
- [x] 2.2.2 Add test for KV cache population in decoder_init (validate shape and values)
- [x] 2.2.3 Add test for position_ids generation (0 to seq_len-1)
- [x] 2.2.4 Add test for greedy_decode selecting highest logit token
- [x] 2.2.5 Add test for decoder_init error handling (ONNX session failures)
- [x] 2.2.6 Add edge case test for empty input_embeds
- [x] 2.2.7 Add edge case test for single token input

### 2.3 Decoder Step Tests

- [x] 2.3.1 Add test for single autoregressive step generating next token
- [x] 2.3.2 Add test for KV cache growth across decoder_step calls
- [x] 2.3.3 Add test for correct position_id usage in decoder_step
- [x] 2.3.4 Add test for embedding lookup for token_id
- [x] 2.3.5 Add test for decoder_step error handling (ONNX failures)
- [x] 2.3.6 Add test for cache consistency (keys and values grow together)

### 2.4 Autoregressive Decode Tests

- [x] 2.4.1 Add test for run_autoregressive_decode generating tokens up to max_tokens
- [x] 2.4.2 Add test for early stopping when IM_END_ID is generated
- [x] 2.4.3 Add test for early stopping when ENDOFTEXT_ID is generated
- [x] 2.4.4 Add test that stop tokens are included in output
- [x] 2.4.5 Add test for error propagation from decoder_step
- [x] 2.4.6 Add edge case test for max_tokens=1
- [x] 2.4.7 Add edge case test for very large max_tokens

### 2.5 Greedy Decode Tests

- [x] 2.5.1 Add test for greedy_decode selecting token with maximum logit value
- [x] 2.5.2 Add test for greedy_decode with all equal logits (selects first)
- [x] 2.5.3 Add test for greedy_decode with negative logits
- [x] 2.5.4 Add test for greedy_decode extracting from last position in sequence

## 3. Prompt Builder Tests

### 3.1 Property-Based Tests

- [x] 3.1.1 Add proptest for prompt structure invariants (always starts with IM_START_ID)
- [x] 3.1.2 Add proptest for audio pad count preservation (exactly n_audio_tokens AUDIO_PAD_ID)
- [x] 3.1.3 Add proptest for matching IM_START_ID and IM_END_ID pairs
- [x] 3.1.4 Add proptest for special token positions (AUDIO_START/AUDIO_END around audio pads)
- [x] 3.1.5 Add proptest for language tokens presence when language is provided

### 3.2 Edge Case Tests

- [x] 3.2.1 Add test for build_prompt_ids with n_audio_tokens=0
- [x] 3.2.2 Add test for build_prompt_ids with very large n_audio_tokens
- [x] 3.2.3 Add test for build_prompt_ids with all supported languages
- [x] 3.2.4 Add test for build_prompt_ids with None language (auto mode)
- [x] 3.2.5 Add test for special token constants validity (verify IDs match Qwen spec)

## 4. Tokenizer Tests

### 4.1 Round-Trip Tests

- [x] 4.1.1 Add test for simple ASCII text round-trip encode/decode
- [x] 4.1.2 Add test for English text with punctuation round-trip
- [x] 4.1.3 Add test for Chinese text round-trip
- [x] 4.1.4 Add test for Japanese text round-trip
- [x] 4.1.5 Add test for Korean text round-trip
- [x] 4.1.6 Add test for mixed language text round-trip
- [x] 4.1.7 Add test for special characters (newlines, tabs) round-trip
- [x] 4.1.8 Add test for emoji handling round-trip

### 4.2 Error Handling Tests

- [x] 4.2.1 Add test for load with missing tokenizer file (clear error message)
- [x] 4.2.2 Add test for encode error handling
- [x] 4.2.3 Add test for decode error handling
- [x] 4.2.4 Add test for invalid UTF-8 handling

## 5. Encoder Tests

### 5.1 Chunking Tests

- [x] 5.1.1 Add test for chunk_mel_spectrogram with frames divisible by CHUNK_SIZE
- [x] 5.1.2 Add test for chunk_mel_spectrogram with frames not divisible by CHUNK_SIZE
- [x] 5.1.3 Add test for chunk_mel_spectrogram with small input (< CHUNK_SIZE)
- [x] 5.1.4 Add test for original_lengths accuracy in chunking
- [x] 5.1.5 Add test for chunk padding to max_chunk_len

### 5.2 Edge Case Tests

- [x] 5.2.1 Add test for compute_conv_output_len with various input lengths
- [x] 5.2.2 Add test for run_encoder with empty mel spectrogram
- [x] 5.2.3 Add test for run_encoder with single frame
- [x] 5.2.4 Add test for run_encoder error handling (ONNX failures)

## 6. Integration and Error Path Tests

### 6.1 Main Engine Tests

- [x] 6.1.1 Add test for transcribe with unsupported language (returns UnsupportedLanguage error)
- [x] 6.1.2 Add test for transcribe with audio too short (< 0.1s)
- [x] 6.1.3 Add test for transcribe with empty audio samples
- [x] 6.1.4 Add test for health_check with missing model files
- [x] 6.1.5 Add test for health_check with all required files present
- [x] 6.1.6 Add test for transcribe_samples error propagation

### 6.2 Integration Test Expansion

- [x] 6.2.1 Add integration test with Chinese audio (if test file available)
- [x] 6.2.2 Add integration test with VAD-enabled long audio chunking
- [x] 6.2.3 Add integration test for streaming transcription behavior
- [x] 6.2.4 Add integration test for various language codes

## 7. Coverage and CI Setup

### 7.1 Coverage Measurement

- [x] 7.1.1 Install `cargo-tarpaulin` if not available
- [x] 7.1.2 Run initial coverage measurement: `cargo tarpaulin --workspace --out Html`
- [x] 7.1.3 Generate HTML coverage report and review uncovered lines
- [x] 7.1.4 Identify gaps in decoder.rs coverage and add tests
- [x] 7.1.5 Identify gaps in prompt.rs coverage and add tests
- [x] 7.1.6 Identify gaps in tokenizer coverage and add tests
- [x] 7.1.7 Verify decoder.rs reaches 90%+ coverage
- [x] 7.1.8 Verify prompt.rs reaches 95%+ coverage
- [x] 7.1.9 Verify tokenizer reaches 90%+ coverage
- [x] 7.1.10 Verify overall stt-qwen3 crate reaches 85%+ coverage

### 7.2 CI Integration

- [x] 7.2.1 Add coverage check to CI configuration (run tarpaulin, fail if below 85%)
- [x] 7.2.2 Add unit test job to CI (fast, no model files required)
- [x] 7.2.3 Separate integration test job (slow, requires model files or marks as optional)
- [x] 7.2.4 Add coverage report upload to CI artifacts
- [x] 7.2.5 Test CI pipeline to ensure tests pass and coverage is measured

## 8. Documentation and Cleanup

- [x] 8.1 Update README with test running instructions
- [x] 8.2 Add documentation for mock session usage in tests
- [x] 8.3 Add examples of how to write new tests using the infrastructure
- [x] 8.4 Review all tests for clarity and add comments if needed
- [x] 8.5 Remove any debug code or temporary test code
- [x] 8.6 Verify `cargo test --all` runs successfully
- [x] 8.7 Verify `cargo clippy --all-targets` passes (or fix warnings)
- [x] 8.8 Verify `cargo fmt --check` passes (or format code)
