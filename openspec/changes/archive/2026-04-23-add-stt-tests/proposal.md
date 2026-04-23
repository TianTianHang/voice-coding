# Proposal: Add Comprehensive STT Inference Tests

## Why

The STT inference pipeline has critical gaps in test coverage. The core decoder module (autoregressive generation, KV cache management) has ZERO tests, while other modules have minimal coverage. This creates significant risk for regressions in core functionality and makes future refactoring dangerous. A comprehensive test suite is needed to ensure reliability as the codebase evolves.

## What Changes

- **Decoder Module Tests**: Add complete unit tests for all decoder functions (`embed_and_fuse`, `decoder_init`, `decoder_step`, `run_autoregressive_decode`, `greedy_decode`)
- **Test Infrastructure**: Create mock ONNX session framework to enable fast unit tests without loading real models
- **Prompt Builder Tests**: Add property-based and edge case tests for `build_prompt_ids`
- **Tokenizer Tests**: Add round-trip encode/decode tests with various text types
- **Encoder Edge Cases**: Add tests for edge cases in encoder chunking and output validation
- **Error Path Testing**: Add tests for error conditions in the main transcription pipeline
- **Test Fixtures**: Create reusable test fixtures and helpers

## Capabilities

### New Capabilities
- `stt-test-coverage`: Comprehensive test coverage for STT inference pipeline including unit, integration, and property-based tests

### Modified Capabilities
None - this is testing infrastructure only, no behavioral changes to production code

## Impact

### Code Structure
- **New Files**: Extensive test modules in `stt-qwen3/src/` (decoder tests, prompt tests, tokenizer tests)
- **New Helpers**: `stt-qwen3/tests/common/` for shared test utilities and mocks
- **Modified Files**: May need minor refactoring to make ONNX sessions injectable for testing

### Dependencies
- **Dev Dependencies**: Add `proptest` for property-based testing, potentially `mockall` for mocking
- **No Runtime Dependencies**: Pure testing addition, no production dependency changes

### Test Execution
- **Unit Tests**: Fast execution (< 1 second) with mocked ONNX sessions
- **Integration Tests**: Slower tests requiring actual model files (already exists, will expand)
- **CI/CD**: Can run unit tests in CI without requiring model downloads

### Coverage Goals
- **Decoder**: Target 90%+ line coverage (currently 0%)
- **Prompt Builder**: Target 95%+ coverage with property tests
- **Tokenizer**: Target 90%+ coverage with round-trip tests
- **Overall Module**: Target 85%+ coverage for stt-qwen3 crate
