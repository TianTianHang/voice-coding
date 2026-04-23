# Design: STT Abstract Layer Implementation

## Context

**Current State:**
- Tauri + React application with no speech-to-text capability
- Single-crate Rust backend structure
- Nix-managed development environment with Rust 1.95.0

**Constraints:**
- Must use Nix for dependency management (no external system dependencies beyond nixpkgs)
- CPU-only inference (no GPU requirement for portability)
- Support for 30 languages with single model
- Minimal runtime overhead through compile-time abstraction

**Stakeholders:**
- Frontend developers: Need simple Tauri command API
- Future maintainers: Need extensible design for adding new STT engines
- Users: Need reliable, fast transcription with privacy (local inference)

## Goals / Non-Goals

**Goals:**
- Create extensible STT abstraction supporting multiple implementations
- Implement production-ready Qwen3-ASR-0.6B ONNX inference pipeline
- Maintain zero-overhead abstraction through compile-time engine selection
- Provide clean async API for Tauri integration
- Support long audio processing through VAD chunking

**Non-Goals:**
- Streaming STT (future enhancement through `StreamingStt` trait)
- Batch processing optimization (future enhancement through `BatchStt` trait)
- GPU acceleration (out of scope for CPU-only requirement)
- Cloud STT API integration (future engines can add this)
- Real-time transcription (focus on file-based transcription)

## Decisions

### Decision 1: Abstract Layer Pattern

**Choice:** Trait-based abstraction with compile-time selection via feature flags

**Rationale:**
- **Zero overhead:** No dynamic dispatch, compiler can inline through type alias
- **Type safety:** Compile-time checking prevents mixing engine implementations
- **Simplicity:** No runtime registration/configuration complexity
- **Binary size:** Unused engines are compiled out entirely

**Alternatives Considered:**
- *Runtime Engine Registry:* Rejected - adds complexity, dynamic dispatch overhead
- *Plugin System:* Rejected - overkill for compile-time use case,安全问题

### Decision 2: Workspace Structure

**Choice:** Three-crate workspace: `stt-core` (traits), `stt-qwen3` (implementation), main crate (Tauri)

**Rationale:**
- **Separation of concerns:** Interface and implementation cleanly separated
- **Reusability:** `stt-core` can be used independently for testing/mocking
- **Independent versioning:** Each crate can evolve independently
- **Clear dependencies:** Implementation depends on core, not vice versa

**Alternatives Considered:**
- *Single Crate:* Rejected - harder to extend, no clear separation
- *Multi-crate with audio-utils:* Rejected - YAGNI, audio can be private to qwen3

### Decision 3: Audio Processing Stack

**Choice:** Pure Rust stack - Symphonia (decoding) + RustFFT (FFT) + custom Mel filterbank

**Rationale:**
- **No FFmpeg dependency:** Symphonia is pure Rust, no system library requirement
- **Nix compatibility:** All dependencies available in nixpkgs
- **Safety:** Rust memory safety vs C library bindings
- **Maintenance:** Pure Rust ecosystem easier to maintain

**Alternatives Considered:**
- *FFmpeg:* Rejected - adds system dependency, larger binary, security surface
- *CPAL:* Rejected - for live audio I/O, not file decoding
- *Python librosa via PyO3:* Rejected - adds Python runtime dependency

### Decision 4: ONNX Runtime Integration

**Choice:** `ort` crate (Rust bindings) with CPU-only execution provider

**Rationale:**
- **Mature library:** Battle-tested bindings for ONNX Runtime
- **CPU optimization:** INT8 quantization support, multi-threading
- **Cross-platform:** Works on Linux/macOS/Windows
- **Nix available:** System library in nixpkgs for FFI dependency

**Alternatives Considered:**
- *Direct ONNX Runtime C API:* Rejected - unsafe, maintenance burden
- *tract:* Rejected - less mature, Qwen3 model compatibility uncertain
- *candle:* Rejected - primarily for PyTorch models, not ONNX

### Decision 5: Async API Design

**Choice:** `async-trait` with `tokio` runtime (already used by Tauri)

**Rationale:**
- **Tauri compatibility:** Tauri commands are async by default
- **Non-blocking:** Long inference doesn't block UI thread
- **Future streaming:** Easy to add streaming support later

**Alternatives Considered:**
- *Sync API:* Rejected - would block Tauri event loop
- *Custom channels:* Rejected - reinventing tokio

### Decision 6: Error Handling Strategy

**Choice:** Structured error type with `thiserror` + `SttError` enum

**Rationale:**
- **Type safety:** Enum variants prevent silent failures
- **Context preservation:** Error chain shows what went wrong
- **Interop:** `std::error::Error` + `Send` + `Sync` for easy wrapping
- **User-friendly:** Can display helpful messages to UI

**Alternatives Considered:**
- * anyhow:* Rejected - loses type information, harder to handle specific errors
- *Box<dyn Error>:* Rejected - no type safety, harder to test

## Architecture

### Module Dependency Graph

```
┌─────────────────────┐
│   Tauri Commands    │
│  (src-tauri/src/)   │
└──────────┬──────────┘
           │ uses
           ↓
┌─────────────────────┐
│   Type Alias        │
│ CurrentSttEngine    │
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    │             │
    ↓             ↓
┌───────────┐ ┌──────────────┐
│ stt-core  │ │ stt-qwen3    │
│  (trait)  │ │ (impl)       │
└───────────┘ └──────┬───────┘
                     │
         ┌───────────┼───────────┐
         ↓           ↓           ↓
    ┌────────┐ ┌─────────┐ ┌──────────┐
    │ Symphonia│ │  ort    │ │rustfft   │
    └────────┘ └─────────┘ └──────────┘
```

### Crate Responsibilities

**stt-core:**
- Define `SttEngine` trait and marker traits (`StreamingStt`, `BatchStt`)
- Define common types: `SttConfig`, `SttResult`, `AudioInput`, `TimingInfo`
- Define error type: `SttError`
- Zero implementation code (pure interface)

**stt-qwen3:**
- Implement `SttEngine` for Qwen3-ASR-0.6B
- Audio preprocessing (loader, Mel spectrogram, VAD)
- ONNX session management (4 models)
- Inference pipeline (encoder, decoder, KV cache)
- Tokenization and prompt building
- Private to implementation (details hidden behind trait)

**src-tauri:**
- Tauri command: `transcribe(audio_path, language?)`
- Global engine instance (lazy_static or once_cell)
- Error translation (SttError → String for Tauri)

### Data Flow

```
Audio File
    │
    ↓
Symphonia Decoder → Raw Samples (f32, 16kHz mono)
    │
    ↓
Mel Spectrogram (rustfft) → 128 bins × N frames
    │
    ↓
ONNX Encoder (Conv + Transformer) → Audio Features [N, 1024]
    │
    ↓
Prompt Builder (tokenizer + audio placeholders) → Token IDs
    │
    ↓
Embedding Fusion (replace audio pads with features) → Input Embeds
    │
    ↓
Decoder Init → Logits + KV Cache
    │
    ↓
Autoregressive Decode Loop → Generated Tokens
    │
    ↓
Tokenizer Decode → Transcribed Text
```

## Risks / Trade-offs

### Risk 1: Mel Spectrogram Numerical Accuracy

**Risk:** Custom Mel implementation may not match librosa exactly, causing accuracy degradation

**Mitigation:**
- Reference Python implementation for exact formulas
- Unit tests comparing against librosa output on known inputs
- Bit-exact comparison for filterbank coefficients
- Accept 1e-5 tolerance for floating point differences

### Risk 2: ONNX Model Compatibility

**Risk:** `ort` crate version may have compatibility issues with Qwen3 ONNX models

**Mitigation:**
- Pin to tested `ort = "2.0"` version
- Test with model files early in implementation
- Fallback to binding system C library if needed
- Model files are stable (exported once, don't change)

### Risk 3: Memory Usage on Long Audio

**Risk:** 10-minute audio consumes 15GB+ without chunking, causing OOM

**Mitigation:**
- Implement VAD-based chunking (split at silence)
- Default 30-second chunks with user override
- Process chunks sequentially, free memory between chunks
- Document 45-second soft limit for single-chunk mode

### Risk 4: Nix Environment Setup

**Risk:** ONNX Runtime system library may not be available or version mismatch

**Mitigation:**
- Verify availability: `nix-instantiate` confirmed onnxruntime in nixpkgs
- Add to flake.nix buildInputs
- Use static linking fallback if needed
- Document setup steps in README

### Risk 5: Performance Regression

**Risk:** Rust implementation slower than Python reference

**Mitigation:**
- Python (RTF 0.32x on desktop) vs Rust target (RTF 0.35x acceptable)
- Profile hot paths with criterion benchmarks
- Optimize Mel computation (likely bottleneck)
- Use rayon for parallel processing where applicable
- INT8 quantization already enabled in models

### Risk 6: Trait Evolution

**Risk:** Adding methods to `SttEngine` trait is breaking change for implementations

**Mitigation:**
- Provide default implementations where possible
- Use marker traits (`StreamingStt`, `BatchStt`) for optional features
- Version crates independently (semver)
- Document that trait is stable but not frozen

## Migration Plan

### Phase 1: Setup (Day 1)
1. Create workspace structure in `src-tauri/`
2. Add `stt-core` and `stt-qwen3` crates
3. Configure feature flags in Cargo.toml
4. Update flake.nix with ONNX Runtime dependency
5. Create placeholder `SttEngine` trait

### Phase 2: Core Implementation (Days 2-4)
1. Implement audio loading with Symphonia
2. Implement Mel spectrogram (STFT + filterbank + log)
3. Add unit tests comparing to librosa output
4. Create ONNX session management
5. Load and verify model files

### Phase 3: Inference Pipeline (Days 5-7)
1. Implement encoder inference
2. Implement decoder with KV cache
3. Add tokenizer integration
4. Build prompt construction logic
5. Implement autoregressive decoding loop

### Phase 4: Integration (Day 8)
1. Create Tauri command wrapper
2. Add error handling
3. Test end-to-end with sample audio
4. Performance benchmarking

### Phase 5: Polish (Day 9)
1. Add VAD chunking for long audio
2. Documentation (API doc, README)
3. Final testing and bug fixes
4. Update pre-commit hooks

### Rollback Strategy
- Feature flag allows disabling STT entirely
- Old code paths remain untouched
- Can revert workspace structure if needed
- Model files separate from source code (easy to remove)

## Open Questions

1. **Model File Distribution:** How to distribute 2.5GB model files?
   - Option A: Git LFS (large file support)
   - Option B: Download script in repository
   - Option C: User manual download
   - **Recommendation:** Download script with checksums verification

2. **Mel Filterbank Constants:** Should we pre-compute and embed as bytes?
   - Pro: Faster startup, no recomputation
   - Con: Binary size increase (~1MB)
   - **Recommendation:** Pre-compute and embed (startup speed matters)

3. **Chunk Size Default:** 30 seconds is default in Python, is this right for Rust?
   - Factors: Memory trade-off, transcription accuracy
   - **Recommendation:** Start with 30s, profile and adjust

4. **Tokenizer Choice:** Use `tokenizers` crate or HuggingFace `transformers-rs`?
   - `tokenizers`: Faster, lighter, used in Python reference
   - `transformers-rs`: Full API, heavier
   - **Recommendation:** `tokenizers` (matches reference)

5. **Error Recovery:** What should happen if inference fails mid-decoding?
   - Option A: Return partial transcription
   - Option B: Return error with no text
   - **Recommendation:** Return error (all-or-nothing for reliability)
