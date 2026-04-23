# Implementation Tasks: STT Abstract Layer

## 1. Project Setup

- [x] 1.1 Convert `src-tauri/` to Cargo workspace with `Cargo.toml` workspace configuration
- [x] 1.2 Create `stt-core/` crate directory structure
- [x] 1.3 Create `stt-qwen3/` crate directory structure
- [x] 1.4 Add workspace dependencies to `src-tauri/Cargo.toml`
- [x] 1.5 Add `async-trait`, `thiserror`, `serde` dependencies to `stt-core/Cargo.toml`
- [x] 1.6 Add `stt-core` dependency to `stt-qwen3/Cargo.toml`
- [x] 1.7 Add ONNX Runtime to Nix flake buildInputs in `flake.nix`
- [x] 1.8 Configure feature flags in `src-tauri/Cargo.toml` (default `stt-qwen3`)
- [x] 1.9 Create placeholder `lib.rs` files for all crates
- [ ] 1.10 Verify workspace builds with `nix develop --command cargo build`

## 2. STT Core Trait Definition

- [x] 2.1 Define `SttEngine` trait in `stt-core/src/traits.rs` with async methods
- [x] 2.2 Add `engine_name()` method returning `&str`
- [x] 2.3 Add `supported_languages()` method returning `&[&str]`
- [x] 2.4 Add `transcribe()` async method accepting `AudioInput` and `SttConfig`
- [x] 2.5 Add `transcribe_batch()` method with default sequential implementation
- [x] 2.6 Add `transcribe_stream()` method returning `NotImplemented` by default
- [x] 2.7 Add `health_check()` async method returning `Result<bool>`
- [x] 2.8 Define marker traits: `StreamingStt` and `BatchStt`
- [x] 2.9 Define `AudioInput` enum (FilePath, Bytes, Samples variants)
- [x] 2.10 Define `SttConfig` struct with language, chunk_seconds, max_new_tokens, enable_vad, detect_language fields
- [x] 2.11 Define `SttResult` struct with text, language, confidence, timing fields
- [x] 2.12 Define `TimingInfo` struct with audio_duration_sec, processing_time_sec, rtf, tokens_generated
- [x] 2.13 Define `SttError` enum with AudioLoadError, InferenceError, TokenizerError, UnsupportedLanguage, NotImplemented, Io, Other variants
- [x] 2.14 Add `thiserror` derives for error display and source chaining
- [x] 2.15 Export all types from `stt-core/src/lib.rs`
- [x] 2.16 Add documentation comments to trait and all public types
- [x] 2.17 Write unit tests for type constructors and error conversion

## 3. Audio Loading Implementation

- [x] 3.1 Create `stt-qwen3/src/audio/` module with mod.rs, loader.rs, mel.rs, vad.rs
- [x] 3.2 Add `symphonia` dependency with "all" features to `stt-qwen3/Cargo.toml`
- [x] 3.3 Implement `load_audio_from_file()` in `loader.rs` using Symphonia
- [x] 3.4 Implement automatic format detection (WAV, MP3, FLAC, OGG, M4A)
- [x] 3.5 Implement resampling to 16kHz using Symphonia's resampler
- [x] 3.6 Implement downmix to mono (average channels if stereo)
- [x] 3.7 Implement `load_audio_from_bytes()` using Cursor and Symphonia
- [x] 3.8 Implement `validate_samples()` for sample rate and duration checks
- [x] 3.9 Return `Vec<f32>` at 16kHz mono float32
- [x] 3.10 Add error handling for file not found, corrupt data, unsupported format
- [x] 3.11 Write unit tests loading various audio formats
- [x] 3.12 Test resampling correctness (48kHz → 16kHz)
- [x] 3.13 Test stereo downmixing
- [x] 3.14 Test minimum duration validation (0.1s)
- [x] 3.15 Test error cases (missing file, invalid format)

## 4. Mel Spectrogram Computation

- [x] 4.1 Add `rustfft` and `dasp` dependencies to `stt-qwen3/Cargo.toml`
- [x] 4.2 Define STFT constants in `mel.rs`: N_FFT=400, HOP_LENGTH=160, N_MELS=128, SAMPLE_RATE=16000
- [x] 4.3 Implement Hann window function
- [x] 4.4 Implement `compute_stft()` using RustFFT
- [x] 4.5 Implement magnitude computation: `magnitude = sqrt(real^2 + imag^2)`
- [x] 4.6 Implement `create_mel_filterbank()` function
- [x] 4.7 Implement Slaney normalization for filterbank
- [x] 4.8 Pre-compute and cache filterbank matrix on engine init
- [x] 4.9 Implement `apply_mel_filterbank()` matrix multiplication
- [x] 4.10 Implement log compression: `log10(max(spec, 1e-10))`
- [x] 4.11 Implement dynamic range clipping: max(log_spec, log_spec.max() - 8.0)
- [x] 4.12 Implement normalization: `(log_spec + 4.0) / 4.0`
- [x] 4.13 Integrate into `compute_mel_spectrogram()` function
- [x] 4.14 Add unit tests for STFT computation
- [x] 4.15 Add tests comparing Mel output to librosa reference values
- [x] 4.16 Validate filterbank coefficients match librosa within 1e-6 tolerance
- [x] 4.17 Validate end-to-end Mel computation within 1e-4 tolerance

## 5. VAD-based Audio Chunking

- [x] 5.1 Implement `compute_rms_energy()` in `vad.rs`
- [x] 5.2 Implement frame-based RMS with 0.2s frame length, 0.1s hop
- [x] 5.3 Implement `rms_to_db()` conversion: `20 * log10(rms / max(rms))`
- [x] 5.4 Implement `detect_silence()` with threshold -40dB
- [x] 5.5 Implement `find_split_points()` with target/2 to target×1.5 range
- [x] 5.6 Implement nearest silent frame selection
- [x] 5.7 Add logic to skip chunking if duration < 45 seconds
- [x] 5.8 Implement `split_audio_at_points()` function
- [x] 5.9 Add tests for silence detection on synthetic audio
- [x] 5.10 Test split point finding on various audio lengths
- [x] 5.11 Test that short audio bypasses chunking

## 6. ONNX Session Management

- [x] 6.1 Add `ort` dependency with "cpu" feature to `stt-qwen3/Cargo.toml`
- [x] 6.2 Create `stt-qwen3/src/models/` module with session.rs
- [x] 6.3 Define `OnnxSessions` struct holding 4 session handles
- [x] 6.4 Implement `load_encoder_conv()` session with CPU provider
- [x] 6.5 Implement `load_encoder_transformer()` session
- [x] 6.6 Implement `load_decoder_init()` session (INT8 or FP32)
- [x] 6.7 Implement `load_decoder_step()` session (INT8 or FP32)
- [x] 6.8 Configure session options: enable all optimizations, set log level
- [x] 6.9 Implement model file path resolution
- [x] 6.10 Add fallback logic: try INT8 models, fall back to FP32 if missing
- [x] 6.11 Return error if any model file fails to load
- [x] 6.12 Implement `load_embeddings()` loading embed_tokens.bin (622MB)
- [x] 6.13 Validate embedding matrix shape [151936, 1024]
- [ ] 6.14 Add tests for session creation (require model files fixture)

## 7. Encoder Inference

- [x] 7.1 Implement `chunk_mel_spectrogram()` splitting into 100-frame chunks
- [x] 7.2 Implement padding to equal chunk lengths
- [x] 7.3 Compute output lengths after Conv stride-2 layers
- [x] 7.4 Prepare input tensor for encoder_conv: [N, 1, 128, L]
- [x] 7.5 Run encoder_conv inference and get output
- [x] 7.6 Implement padding removal from each chunk
- [x] 7.7 Concatenate chunks along sequence dimension
- [x] 7.8 Prepare attention mask [1, 1, total_tokens, total_tokens]
- [x] 7.9 Prepare hidden_states input [total_tokens, 896]
- [x] 7.10 Run encoder_transformer inference
- [x] 7.11 Extract output [total_tokens, 1024]
- [ ] 7.12 Test encoder with synthetic Mel input
- [ ] 7.13 Validate output shapes match expected dimensions

## 8. Tokenizer Integration

- [x] 8.1 Add `tokenizers` dependency to `stt-qwen3/Cargo.toml`
- [x] 8.2 Create `stt-qwen3/src/tokenizer/` module
- [x] 8.3 Implement `TokenizerWrapper` struct
- [x] 8.4 Load tokenizer.json file in constructor
- [x] 8.5 Implement `encode()` method returning Vec<u32>
- [x] 8.6 Implement `decode()` method returning String
- [x] 8.7 Handle special tokens correctly in decode
- [ ] 8.8 Add tests for encoding common phrases
- [ ] 8.9 Test round-trip: encode then decode

## 9. Prompt Construction

- [x] 9.1 Define special token ID constants in `stt-qwen3/src/prompt.rs`
- [x] 9.2 Implement `build_prompt_ids()` function
- [x] 9.3 Add system message: IM_START + "system" + NEWLINE + IM_END + NEWLINE
- [x] 9.4 Add user message: IM_START + "user" + NEWLINE
- [x] 9.5 Add audio placeholders: AUDIO_START + N×AUDIO_PAD + AUDIO_END
- [x] 9.6 Add IM_END + NEWLINE
- [x] 9.7 Add assistant: IM_START + "assistant" + NEWLINE
- [x] 9.8 Optionally add language spec: "language {lang}<asr_text>"
- [x] 9.9 Return Vec<u32> of token IDs
- [ ] 9.10 Test prompt structure with known audio token count
- [ ] 9.11 Test with and without language specification

## 10. Embedding Fusion

- [x] 10.1 Implement `embed_tokens()` lookup from embedding matrix
- [x] 10.2 Create embeddings array [seq_len, 1024] from token IDs
- [x] 10.3 Find AUDIO_PAD_ID positions in token_ids
- [x] 10.4 Validate encoder output shape matches AUDIO_PAD count
- [x] 10.5 Replace audio_pad embeddings with encoder features
- [x] 10.6 Add batch dimension [1, seq_len, 1024]
- [x] 10.7 Return fused embeddings for decoder
- [ ] 10.8 Test fusion with synthetic encoder output
- [ ] 10.9 Test shape mismatch error case

## 11. Decoder with KV Cache

- [x] 11.1 Implement `decoder_init()` call with input_embeds and position_ids
- [x] 11.2 Prepare input embeddings [1, seq_len, 1024]
- [x] 11.3 Prepare position_ids [0, 1, ..., seq_len-1]
- [x] 11.4 Run decoder_init inference
- [x] 11.5 Extract logits [1, seq_len, vocab_size]
- [x] 11.6 Extract present_keys and present_values
- [x] 11.7 Implement KV cache struct to store keys/values
- [x] 11.8 Implement `greedy_decode_token()` taking argmax of last logits
- [x] 11.9 Implement `decoder_step()` loop
- [x] 11.10 Embed next_token: [1, 1, 1024]
- [x] 11.11 Prepare position_ids [[cur_pos]]
- [x] 11.12 Pass past_keys and past_values from KV cache
- [x] 11.13 Run decoder_step inference
- [x] 11.14 Extract updated keys/values
- [x] 11.15 Update KV cache with new tokens
- [x] 11.16 Implement stop conditions: IM_END, ENDOFTEXT, max_tokens
- [x] 11.17 Remove stop tokens from final output
- [ ] 11.18 Test decoder init with synthetic prompt
- [ ] 11.19 Test autoregressive loop for 10-20 steps
- [ ] 11.20 Test stop conditions

## 12. Qwen3AsrEngine Implementation

- [x] 12.1 Create `Qwen3AsrEngine` struct in `stt-qwen3/src/lib.rs`
- [x] 12.2 Add fields: model_dir, sessions, embeddings, tokenizer, mel_filterbank
- [x] 12.3 Implement `new()` constructor loading all models and resources
- [x] 12.4 Implement `SttEngine` trait for Qwen3AsrEngine
- [x] 12.5 Implement `engine_name()` returning "qwen3-asr-0.6b"
- [x] 12.6 Implement `supported_languages()` returning 30 language codes
- [x] 12.7 Implement `health_check()` verifying all files exist
- [x] 12.8 Implement `transcribe()` orchestrating full pipeline
- [x] 12.9 Add VAD chunking branch for long audio
- [x] 12.10 Process chunks sequentially and concatenate results
- [x] 12.11 Add timing measurements for each stage
- [x] 12.12 Calculate RTF and populate TimingInfo
- [x] 12.13 Handle all error cases with descriptive messages
- [ ] 12.14 Test with short audio (no chunking)
- [ ] 12.15 Test with long audio (VAD chunking)

## 13. Tauri Integration

- [x] 13.1 Create `src-tauri/src/asr.rs` module
- [x] 13.2 Add type alias: `type CurrentSttEngine = stt_qwen3::Qwen3AsrEngine`
- [x] 13.3 Create global engine instance using `once_cell::sync::Lazy`
- [x] 13.4 Initialize engine with model directory path
- [x] 13.5 Define `transcribe` Tauri command with async fn
- [x] 13.6 Accept audio_path: String and language: Option<String> parameters
- [x] 13.7 Convert parameters to AudioInput and SttConfig
- [x] 13.8 Call engine.transcribe() and await result
- [x] 13.9 Convert SttError to String for Tauri
- [x] 13.10 Return transcribed text as String
- [x] 13.11 Register command in lib.rs invoke_handler
- [x] 13.12 Update `src-tauri/Cargo.toml` with stt-qwen3 dependency
- [ ] 13.13 Test command with Tauri dev server
- [ ] 13.14 Test error handling (missing file, unsupported language)

## 14. Model Files Setup

- [x] 14.1 Create `scripts/download_model.sh` script
- [x] 14.2 Use git clone to download from HuggingFace
- [x] 14.3 Add checksum verification for all model files
- [x] 14.4 Verify file sizes match expected values
- [ ] 14.5 Test download script on clean environment
- [x] 14.6 Add .gitignore entry for model directory (2.5GB)
- [ ] 14.7 Document download process in README
- [x] 14.8 Add model path configuration to environment variables or config file

## 15. Testing and Validation

- [ ] 15.1 Create test fixtures directory with sample audio files
- [ ] 15.2 Add short audio (~5s) in WAV format for basic tests
- [ ] 15.3 Add long audio (~60s) for VAD chunking tests
- [ ] 15.4 Write integration test for full pipeline
- [ ] 15.5 Test transcription accuracy (compare to expected text)
- [ ] 15.6 Test language parameter (specify "en", "zh", etc.)
- [ ] 15.7 Test error cases (empty audio, corrupt file)
- [ ] 15.8 Benchmark performance on desktop CPU
- [ ] 15.9 Verify RTF < 0.35 on desktop
- [ ] 15.10 Profile memory usage during transcription
- [ ] 15.11 Verify memory < 6GB with VAD chunking
- [ ] 15.12 Test concurrent transcriptions (if using thread-safe engine)

## 16. Documentation

- [ ] 16.1 Add rustdoc comments to all public APIs
- [ ] 16.2 Document `SttEngine` trait with examples
- [ ] 16.3 Document `Qwen3AsrEngine` usage
- [ ] 16.4 Create README in `stt-core/` explaining the abstraction
- [ ] 16.5 Create README in `stt-qwen3/` with model details
- [ ] 16.6 Update main README with STT feature description
- [ ] 16.7 Document model download process
- [ ] 16.8 Document Nix environment setup
- [ ] 16.9 Add troubleshooting section for common issues
- [ ] 16.10 Document performance characteristics and requirements

## 17. Code Quality

- [ ] 17.1 Run `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] 17.2 Fix all clippy warnings
- [ ] 17.3 Run `cargo fmt` on all crates
- [ ] 17.4 Ensure all tests pass: `cargo test --all`
- [ ] 17.5 Check documentation builds: `cargo doc --no-deps --open`
- [ ] 17.6 Add pre-commit hook for clippy and tests if not already present
- [ ] 17.7 Verify feature flags work correctly
- [ ] 17.8 Test build with `--no-default-features`
- [ ] 17.9 Test build with `--features stt-qwen3`

## 18. Performance Optimization

- [ ] 18.1 Profile Mel spectrogram computation with criterion
- [ ] 18.2 Optimize hot loops if RTF > 0.35 target
- [ ] 18.3 Test parallel processing for audio chunks using rayon
- [ ] 18.4 Benchmark with/without parallel chunks
- [ ] 18.5 Optimize memory allocations in inference loop
- [ ] 18.6 Reuse tensors where possible
- [ ] 18.7 Profile decoder autoregressive loop
- [ ] 18.8 Test different thread counts for ONNX sessions
- [ ] 18.9 Document final performance characteristics

## 19. Final Polish

- [ ] 19.1 Test on target CPU architectures (x86_64, ARM if applicable)
- [ ] 19.2 Test with various audio formats and qualities
- [ ] 19.3 Test with all 30 supported languages
- [ ] 19.4 Verify VAD chunking works on very long audio (5+ minutes)
- [ ] 19.5 Add logging for debugging (optional feature flag)
- [ ] 19.6 Clean up debug code and comments
- [ ] 19.7 Run final integration test suite
- [ ] 19.8 Update CHANGELOG if applicable
- [ ] 19.9 Tag version for release
- [ ] 19.10 Prepare demo for showcase
