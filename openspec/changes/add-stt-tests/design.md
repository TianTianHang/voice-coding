# Design: STT Test Infrastructure

## Context

### Current State

The `stt-qwen3` crate implements a complex STT inference pipeline with multiple stages:
- **Audio processing**: Loading, resampling, VAD, mel spectrogram computation
- **Encoder**: ONNX conv + transformer layers
- **Decoder**: Autoregressive generation with KV cache
- **Tokenizer**: Tokenization/detokenization

**Test coverage现状:**
- ✅ `audio/mel.rs`: Good coverage (window, STFT, filterbank, log compression)
- ✅ `audio/vad.rs`: Good coverage (RMS, silence detection, splitting)
- ✅ `audio/loader.rs`: Good coverage (validation, resampling, error paths)
- ✅ `encoder.rs`: Basic coverage (chunking, conv output length)
- ⚠️  `prompt.rs`: Only constant tests, no actual prompt building tests
- ⚠️  `tokenizer/wrapper.rs`: Only missing file test
- ❌ `decoder.rs`: **ZERO tests** - core generation logic completely untested
- ⚠️  `lib.rs`: Only integration test, no unit tests for error paths

### Constraints

- **ONNX Runtime**: Real ONNX sessions require 2.5GB model files and ~5GB RAM
- **Test Speed**: Integration tests with real models are slow (~10s per test)
- **CI Environment**: Model files may not be available in all CI environments
- **Rust Testing**: Standard Rust unit tests with cargo test

### Stakeholders

- **Developers**: Need fast feedback loop, confidence in refactoring
- **CI/CD**: Need fast tests that run without external dependencies
- **Users**: Need reliable transcription without regressions

## Goals / Non-Goals

**Goals:**

1. **90%+ coverage** for decoder module (currently 0%)
2. **Fast unit tests** that run without model files (< 1 second total)
3. **Property-based tests** for prompt builder invariants
4. **Round-trip tests** for tokenizer correctness
5. **Error path coverage** for all public APIs
6. **Reusable test infrastructure** for future STT engines

**Non-Goals:**

- ❌ Modifying production code behavior (tests only, no functional changes)
- ❌ Adding runtime dependencies (dev dependencies only)
- ❌ Replacing integration tests (unit tests complement, not replace)
- ❌ Testing ONNX Runtime itself (trust library, test our usage)
- ❌ Performance benchmarking (separate concern)

## Decisions

### 1. Mock ONNX Sessions with Trait Abstraction

**Decision:** Extract ONNX session operations into a `SessionManager` trait and provide mock implementation for tests.

**Rationale:**
- **Pros:**
  - Fast tests (< 100ms per test vs ~10s with real models)
  - Deterministic outputs (no randomness from model inference)
  - No external dependencies (runs anywhere)
  - Enables edge case testing (errors, weird tensor shapes)
- **Cons:**
  - Requires minor refactoring to inject trait
  - Mock must be kept in sync with real session interface
- **Alternatives considered:**
  - **Real models only**: Too slow for rapid iteration
  - **Conditional compilation**: Test code would diverge from prod
  - **Snapshot testing**: Good for regression, but doesn't test logic

**Implementation:**
```rust
// In stt-core or new stt-testing crate
pub trait SessionManager {
    fn run_encoder(&mut self, mel: &[Vec<f64>]) -> Result<Vec<Vec<f32>>, SttError>;
    fn run_decoder_init(&mut self, input_embeds: &Array3<f32>) -> Result<(u32, KvCache), SttError>;
    fn run_decoder_step(&mut self, ...) -> Result<(u32, KvCache), SttError>;
}

// Production implementation wraps real ONNX sessions
// Mock implementation for tests returns pre-canned responses
```

### 2. Property-Based Testing for Prompt Builder

**Decision:** Use `proptest` crate for property-based testing of `build_prompt_ids`.

**Rationale:**
- **Why proptest:**
  - Automatically finds edge cases (empty, min, max values)
  - Shrinks failing cases to minimal counterexamples
  - Native Rust integration
- **What to test:**
  - Invariants: Structure always valid regardless of n_audio_tokens
  - Properties: Audio pad count matches input, special tokens in correct positions
- **Alternatives considered:**
  - Manual edge cases: Easy to miss corner cases
  - QuickCheck: Older, less maintained

**Example property:**
```rust
proptest! {
  #[test]
  fn prop_prompt_audio_token_count(n_audio_tokens in 0..1000usize) {
    let ids = build_prompt_ids(n_audio_tokens, None, &tokenizer).unwrap();
    let audio_pad_count = ids.iter().filter(|&&id| id == AUDIO_PAD_ID).count();
    assert_eq!(audio_pad_count, n_audio_tokens);
  }
}
```

### 3. Test Structure: Module-level `tests` submodule

**Decision:** Place tests in `#[cfg(test)]` modules within each source file.

**Rationale:**
- **Pros:**
  - Tests close to code (easy to find, easy to update)
  - Access to private items (can test internal functions)
  - Standard Rust convention
- **Cons:**
  - Increases source file size
- **Alternatives considered:**
  - Separate `tests/` directory: Better for integration tests, not unit tests
  - `__tests__` folder (TypeScript style): Not Rust convention

**Structure:**
```
stt-qwen3/src/
├── decoder.rs
│   └── #[cfg(test)] mod tests { /* decoder unit tests */ }
├── prompt.rs
│   └── #[cfg(test)] mod tests { /* prompt tests + proptest */ }
└── tokenizer/
    └── wrapper.rs
        └── #[cfg(test)] mod tests { /* tokenizer tests */ }
```

### 4. Shared Test Fixtures in `tests/common/`

**Decision:** Create shared test utilities in `stt-qwen3/tests/common/` for integration tests and complex fixtures.

**Rationale:**
- Integration tests need shared model-loading logic
- Mock session builders used across multiple modules
- Test data generators (mel spectrograms, audio samples)

**Structure:**
```
stt-qwen3/tests/
├── common/
│   ├── mod.rs
│   ├── mock_sessions.rs  // Mock SessionManager implementations
│   └── fixtures.rs       // Test data generators
├── integration_test.rs   // Existing integration test
└── decoder_integration.rs // Future: focused integration tests
```

### 5. Coverage Tooling: `tarpaulin` with HTML reports

**Decision:** Use `cargo-tarpaulin` for coverage measurement with HTML reports.

**Rationale:**
- **Why tarpaulin:**
  - Works with stable Rust (unlike llvm-cov)
  - Generates HTML reports for visual inspection
  - CI-friendly (exits with non-zero if below threshold)
- **Alternatives considered:**
  - `cargo-llvm-cov`: More accurate but requires nightly
  - `grcov`: Complex setup, harder to use

**Configuration:**
```toml
# In .cargo/config.toml or CI
[tarpaulin]
out = ["Html"]
fail-under = 85  # Enforce 85% overall coverage
```

## Risks / Trade-offs

### Risk 1: Mock Implementation Drifts from Real Sessions

**Risk:** Mock ONNX sessions may not accurately simulate real session behavior, leading to tests that pass but code fails in production.

**Mitigation:**
- Keep mock implementation minimal but realistic
- Add integration tests with real models to validate end-to-end
- Run integration tests in CI weekly or before releases
- Document mock behavior assumptions

### Risk 2: Test Maintenance Burden

**Risk:** More tests = more maintenance when refactoring.

**Mitigation:**
- Focus on testing behavior, not implementation details
- Use helper functions to reduce test duplication
- Make tests readable and self-documenting
- Prune obsolete tests during refactoring

### Risk 3: Property Tests Finding Too Many Edge Cases

**Risk:** Proptest may find many edge cases that are actually bugs, overwhelming development.

**Mitigation:**
- Start with reasonable input ranges (0..1000, not 0..usize::MAX)
- Use `proptest`'s case rejection to filter truly invalid inputs
- Fix legitimate bugs as they're found
- Document known limitations

### Trade-off 1: Test Speed vs. Realism

- **Fast unit tests (mocks)**: Good iteration speed, may miss real bugs
- **Slow integration tests (real models)**: Catch real bugs, slow iteration
- **Balance**: 80% unit tests, 20% integration tests

### Trade-off 2: Coverage vs. Development Time

- **Higher coverage**: More confidence, slower initial development
- **Lower coverage**: Faster initial development, more bugs
- **Balance**: Target 85% overall, 90%+ for critical paths

## Migration Plan

### Phase 1: Test Infrastructure (Week 1)

1. Add dev dependencies to `Cargo.toml`:
   ```toml
   [dev-dependencies]
   proptest = "1.4"
   mockall = "0.12"  # For mocking SessionManager
   ```

2. Create `SessionManager` trait in `stt-core` or new `stt-testing` crate

3. Implement `MockSessionManager` in `tests/common/mock_sessions.rs`

4. Refactor production code to use `SessionManager` trait (minimal changes)

### Phase 2: Decoder Tests (Week 1-2)

1. Add unit tests for `embed_and_fuse` (edge cases, validation)
2. Add unit tests for `decoder_init` (mock session, cache validation)
3. Add unit tests for `decoder_step` (cache growth, token generation)
4. Add unit tests for `greedy_decode` (correct token selection)
5. Add unit tests for `run_autoregressive_decode` (stop conditions)

### Phase 3: Prompt and Tokenizer Tests (Week 2)

1. Add property-based tests for `build_prompt_ids`
2. Add round-trip tests for tokenizer
3. Add edge case tests (empty input, special characters)

### Phase 4: Encoder and Integration Tests (Week 2-3)

1. Add encoder edge case tests (chunking, boundaries)
2. Expand integration test suite with more test cases
3. Add error path tests for `lib.rs` (invalid language, audio errors)

### Phase 5: Coverage and CI (Week 3)

1. Run `cargo tarpaulin --out Html` to measure coverage
2. Address gaps to reach 85%+ overall coverage
3. Add coverage check to CI (pre-merge gate)
4. Document test running in README

### Rollback Strategy

- Tests don't affect production code - can be rolled back independently
- If mock approach fails, can remove trait and use integration tests only
- All new code is in `#[cfg(test)]` - zero runtime impact

## Open Questions

1. **SessionManager location:** Should it go in `stt-core` (shared across engines) or `stt-qwen3` (engine-specific)?
   - **Leaning:** `stt-core` for future engine reuse

2. **Property test input ranges:** What are reasonable bounds for `n_audio_tokens`, `max_tokens`, etc.?
   - **Need to research:** Typical audio lengths, model limits

3. **Integration test frequency:** Should real-model tests run on every PR or only before releases?
   - **Leaning:** Every PR but marked as `//ignore-ci` to be optional, or run in separate slower CI job

4. **Mock session fidelity:** How closely should mock sessions simulate real ONNX outputs (tensor shapes, value ranges)?
   - **Need to prototype:** Start with simple mocks, add realism as needed
