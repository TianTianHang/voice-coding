# Spec: Qwen3 ASR Implementation

## ADDED Requirements

### Requirement: Implement SttEngine trait for Qwen3-ASR-0.6B

The system SHALL provide `Qwen3AsrEngine` implementation of the `SttEngine` trait for Qwen3-ASR-0.6B ONNX model.

#### Scenario: Engine identification

- **WHEN** `engine_name()` is called
- **THEN** it SHALL return `"qwen3-asr-0.6b"`
- **AND** it SHALL uniquely identify this implementation

#### Scenario: Language support

- **WHEN** `supported_languages()` is called
- **THEN** it SHALL return array of 30 language codes
- **AND** it MUST include: `"zh"`, `"en"`, `"yue"`, `"ja"`, `"ko"`, `"ar"`, `"de"`, `"fr"`, `"es"`, `"pt"`
- **AND** it MUST include: `"id"`, `"it"`, `"ru"`, `"th"`, `"vi"`, `"tr"`, `"hi"`, `"ms"`, `"nl"`, `"sv"`
- **AND** it MUST include: `"da"`, `"fi"`, `"pl"`, `"cz"`, `"fil"`, `"fa"`, `"el"`, `"ro"`, `"hu"`, `"mk"`

#### Scenario: Health check verifies model files

- **WHEN** `health_check()` is called
- **THEN** it SHALL verify all 4 ONNX model files exist
- **AND** it MUST verify `embed_tokens.bin` exists
- **AND** it MUST verify `tokenizer.json` exists
- **AND** it SHALL return error if any file is missing

### Requirement: Load and initialize ONNX models

The system SHALL load 4 ONNX model files from the specified model directory during engine initialization.

#### Scenario: Model file paths

- **WHEN** initializing with model directory path
- **THEN** it SHALL load `encoder_conv.onnx` from `onnx_models/` subdirectory
- **AND** it SHALL load `encoder.int4.onnx` from `onnx_models/` subdirectory
- **AND** it SHALL load `decoder_init.int4.onnx` from `onnx_models/` subdirectory
- **AND** it SHALL load `decoder_step.int4.onnx` from `onnx_models/` subdirectory

> Note: the load list above reflects the updated ONNX export. The old `encoder_conv` / `encoder_transformer` split is no longer used.

#### Scenario: ONNX session configuration

- **WHEN** creating ONNX inference sessions
- **THEN** it SHALL use CPU execution provider
- **AND** it SHALL enable all graph optimizations
- **AND** it MAY set intra-op num threads if specified in config
- **AND** it SHALL set log severity to suppress warnings

#### Scenario: Embedding matrix loading

- **WHEN** loading `embed_tokens.bin`
- **THEN** it SHALL read binary file as float32 array
- **AND** it MUST reshape to `[VOCAB_SIZE, HIDDEN_SIZE]` = `[151936, 1024]`
- **AND** it SHALL validate file size matches expected dimensions (~622MB)

#### Scenario: Initialization failure

- **WHEN** model file is missing or corrupted
- **THEN** initialization SHALL return `InferenceError`
- **AND** error message MUST specify which file failed to load

### Requirement: Transcribe audio with full pipeline

The system SHALL implement `transcribe()` method executing the complete Qwen3-ASR inference pipeline.

#### Scenario: End-to-end transcription

- **WHEN** `transcribe()` is called with audio file path
- **THEN** it SHALL load and decode audio file to 16kHz mono float32 samples
- **AND** it SHALL compute Mel spectrogram (128 bins)
- **AND** it SHALL run encoder (conv + transformer)
- **AND** it SHALL build prompt with audio tokens
- **AND** it SHALL run decoder init (prefill)
- **AND** it SHALL run autoregressive decode loop
- **AND** it SHALL decode tokens to text
- **AND** it SHALL return `SttResult` with transcribed text

#### Scenario: Language parameter

- **WHEN** config specifies `language: Some("zh")`
- **THEN** prompt SHALL include `"language zh<asr_text>"`
- **AND** decoder SHALL produce Chinese text
- **AND** result `language` field SHALL be `"zh"`

#### Scenario: Auto language detection

- **WHEN** config specifies `language: None` and `detect_language: true`
- **THEN** system SHALL detect language from audio
- **AND** result `language` field SHALL contain detected code

#### Scenario: Processing time tracking

- **WHEN** transcription completes
- **THEN** `timing.audio_duration_sec` SHALL equal input audio length
- **AND** `timing.processing_time_sec` SHALL measure wall-clock time
- **AND** `timing.rtf` SHALL be < 1.0 on desktop CPU (faster than realtime)
- **AND** `timing.tokens_generated` SHALL count output tokens

### Requirement: Support VAD-based audio chunking

The system SHALL automatically split long audio files at silence boundaries to avoid out-of-memory errors.

#### Scenario: Short audio (no chunking)

- **WHEN** audio duration < 45 seconds
- **THEN** system SHALL process as single chunk
- **AND** it SHALL NOT perform VAD splitting

#### Scenario: Long audio (VAD chunking)

- **WHEN** audio duration ≥ 45 seconds and `enable_vad: true`
- **THEN** system SHALL detect silence boundaries using RMS energy
- **AND** it SHALL split at nearest silent frame to target chunk size
- **AND** default target SHALL be 30 seconds (configurable via `chunk_seconds`)
- **AND** it SHALL process each chunk independently
- **AND** it SHALL concatenate chunk results with spaces

#### Scenario: Custom chunk duration

- **WHEN** config specifies `chunk_seconds: Some(20)`
- **THEN** target split point SHALL be 20 seconds
- **AND** minimum chunk SHALL be 10 seconds (target/2)
- **AND** maximum chunk SHALL be 30 seconds (target×1.5)

#### Scenario: VAD parameters

- **WHEN** detecting silence for splitting
- **THEN** silence threshold SHALL be -40dB
- **AND** hop size for RMS calculation SHALL be 0.1 seconds
- **AND** it SHALL use frame length of 2× hop size

### Requirement: Mel spectrogram computation

The system SHALL compute log-Mel spectrogram matching librosa's implementation for model compatibility.

#### Scenario: STFT computation

- **WHEN** computing Mel spectrogram from audio samples
- **THEN** it SHALL use Hann window
- **AND** n_fft SHALL be 400
- **AND** hop_length SHALL be 160
- **AND** it SHALL center the window
- **AND** pad mode SHALL be "reflect"

#### Scenario: Mel filterbank

- **WHEN** applying Mel filterbank to magnitude spectrogram
- **THEN** it SHALL use 128 Mel bins
- **AND** sample rate SHALL be 16000
- **AND** frequency range SHALL be 0 to 8000 Hz (Nyquist)
- **AND** normalization SHALL be "slaney"
- **AND** htk parameter SHALL be false

#### Scenario: Log compression and normalization

- **WHEN** computing log spectrogram
- **THEN** it SHALL apply log10(max(spec, 1e-10))
- **AND** it SHALL clip values to max-8.0 range
- **AND** it SHALL normalize to [-1, 1] via `(log + 4.0) / 4.0`
- **AND** output dtype SHALL be float32

#### Scenario: Output format

- **WHEN** Mel spectrogram computation completes
- **THEN** output shape SHALL be `[n_mels, n_frames]` = `[128, n_frames]`
- **AND** n_frames SHALL be approximately audio_samples / hop_length

### Requirement: Encoder inference pipeline

The system SHALL run Qwen3-ASR encoder through Conv and Transformer ONNX models.

#### Scenario: Chunked Conv processing

- **WHEN** Mel spectrogram length exceeds chunk size (100 frames)
- **THEN** it SHALL split into chunks of 100 frames
- **AND** it SHALL pad chunks to equal length
- **AND** it SHALL run `encoder.int4.onnx` on audio frames
- **AND** it SHALL remove padding from output

#### Scenario: Conv output length calculation

- **WHEN** calculating output lengths after Conv
- **THEN** for each stride-2 layer (3 layers total): `length = (length - 1) // 2 + 1`
- **AND** it SHALL apply this formula to each chunk independently

#### Scenario: Transformer attention

- **WHEN** running `encoder.int4.onnx`
- **THEN** it SHALL pass hidden states from Conv
- **AND** it SHALL construct all-to-all attention mask (no windowing on CPU)
- **AND** output shape SHALL be `[total_tokens, 1024]`
- **AND** total_tokens SHALL equal sum of Conv output lengths

### Requirement: Prompt construction

The system SHALL build prompt token IDs following Qwen3 chat template with audio placeholders.

#### Scenario: Full prompt structure

- **WHEN** building prompt for N audio tokens
- **THEN** token IDs SHALL follow structure:
  - `[IM_START_ID] + tokenize("system") + [NEWLINE_ID, IM_END_ID, NEWLINE_ID]`
  - `+ [IM_START_ID] + tokenize("user") + [NEWLINE_ID]`
  - `+ [AUDIO_START_ID] + [AUDIO_PAD_ID] * N + [AUDIO_END_ID]`
  - `+ [IM_END_ID, NEWLINE_ID]`
  - `+ [IM_START_ID] + tokenize("assistant") + [NEWLINE_ID]`
  - `+ (optional) tokenize("language {lang}<asr_text>")`

#### Scenario: Special token IDs

- **WHEN** constructing prompt
- **THEN** AUDIO_START_ID SHALL be 151669
- **AND** AUDIO_END_ID SHALL be 151670
- **AND** AUDIO_PAD_ID SHALL be 151676
- **AND** IM_START_ID SHALL be 151644
- **AND** IM_END_ID SHALL be 151645
- **AND** NEWLINE_ID SHALL be 198

#### Scenario: Language specification

- **WHEN** config specifies language
- **THEN** prompt SHALL include `tokenize("language {lang}<asr_text>")` after assistant start
- **AND** {lang} SHALL be replaced with language code

### Requirement: Embedding fusion

The system SHALL embed token IDs and replace audio placeholder positions with encoder output features.

#### Scenario: Token embedding lookup

- **WHEN** embedding prompt token IDs
- **THEN** it SHALL use loaded `embed_tokens` matrix [151936, 1024]
- **AND** it SHALL perform matrix indexing: embeddings = embed_tokens[token_ids]
- **AND** output shape SHALL be `[seq_len, 1024]`

#### Scenario: Audio feature replacement

- **WHEN** fusing audio features
- **THEN** it SHALL identify positions where token_id == AUDIO_PAD_ID
- **AND** it SHALL replace those rows with encoder output features
- **AND** encoder output shape `[N, 1024]` MUST match number of AUDIO_PAD_ID tokens
- **AND** final shape SHALL be `[1, seq_len, 1024]` (batch dimension added)

### Requirement: Decoder with KV cache

The system SHALL run autoregressive decoder using separate init and step ONNX models with KV cache optimization.

#### Scenario: Decoder init (prefill)

- **WHEN** running decoder init
- **THEN** it SHALL pass input embeddings [1, seq_len, 1024]
- **AND** it SHALL pass position_ids [0, 1, 2, ..., seq_len-1]
- **AND** it SHALL output logits [1, seq_len, vocab_size]
- **AND** it SHALL output present_keys [num_layers, batch, kv_heads, seq_len, head_dim]
- **AND** it SHALL output present_values [num_layers, batch, kv_heads, seq_len, head_dim]

#### Scenario: Greedy decoding

- **WHEN** selecting next token from logits
- **THEN** it SHALL take argmax of last logits vector: `next_token = argmax(logits[0, -1, :])`
- **AND** it SHALL append to generated sequence

#### Scenario: Decoder step (autoregressive)

- **WHEN** generating subsequent tokens
- **THEN** it SHALL embed next_token: [1, 1, 1024]
- **AND** it SHALL pass position_ids [[cur_pos]]
- **AND** it SHALL pass past_keys and past_values from previous step
- **AND** model SHALL extend KV cache with new token
- **AND** output logits SHALL be [1, 1, vocab_size] (only last position)
- **AND** output keys/values SHALL have extended seq_len

#### Scenario: Stop conditions

- **WHEN** decoding
- **THEN** it SHALL stop if token == IM_END_ID (151645)
- **AND** it SHALL stop if token == ENDOFTEXT_ID (151643)
- **AND** it SHALL stop if max_new_tokens reached
- **AND** final output SHALL NOT include stop tokens

### Requirement: Tokenization integration

The system SHALL use tokenizer.json for encoding and decoding text.

#### Scenario: Load tokenizer

- **WHEN** initializing engine
- **THEN** it SHALL load `tokenizer.json` from model directory
- **AND** it SHALL use `tokenizers` crate for deserialization

#### Scenario: Encode text

- **WHEN** encoding text to token IDs
- **THEN** it SHALL use tokenizer.encode(text, add_special_tokens=false)
- **AND** it SHALL return list of u32 token IDs

#### Scenario: Decode tokens

- **WHEN** decoding generated tokens to text
- **THEN** it SHALL use tokenizer.decode(tokens, skip_special_tokens=true)
- **AND** it SHALL strip special tokens from output
- **AND** it SHALL return String

### Requirement: Performance targets

The system SHALL meet or exceed reference Python implementation performance.

#### Scenario: Desktop performance

- **WHEN** running on x86_64 desktop CPU
- **THEN** RTF SHALL be ≤ 0.35 (3x faster than realtime)
- **AND** memory usage SHALL be ≤ 6GB peak with VAD chunking

#### Scenario: Low-power performance

- **WHEN** running on Intel N100 (8W TDP)
- **THEN** RTF SHALL be ≤ 0.71 with VAD chunking
- **AND** memory usage SHALL be ≤ 6GB with chunking

#### Scenario: Token generation speed

- **WHEN** decoding tokens (INT8 model)
- **THEN** generation speed SHALL be ~100ms per token
- **AND** it SHALL use multi-threading if available

### Requirement: Error handling for Qwen3-specific failures

The system SHALL provide detailed error messages for Qwen3-ASR specific failure modes.

#### Scenario: Dimension mismatch

- **WHEN** encoder output shape doesn't match audio token count
- **THEN** it SHALL return `InferenceError` with shape details
- **AND** message SHALL include expected vs actual dimensions

#### Scenario: Empty audio

- **WHEN** input audio has duration < 0.1 seconds
- **THEN** it SHALL return `AudioLoadError` with message about minimum length

#### Scenario: Tokenizer failure

- **WHEN** tokenizer fails to encode/decode
- **THEN** it SHALL return `TokenizerError` with underlying error

#### Scenario: ONNX runtime error

- **WHEN** ONNX inference fails
- **THEN** it SHALL return `InferenceError` with ONNX error message
- **AND** it SHALL include model name and operation type (init/step)
