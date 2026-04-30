# Spec: ONNX Inference

## ADDED Requirements

### Requirement: Create ONNX inference sessions

The system SHALL create and manage ONNX Runtime inference sessions for all Qwen3-ASR models.

#### Scenario: Session creation

- **WHEN** initializing ONNX sessions
- **THEN** it SHALL create one session per model file (4 sessions total)
- **AND** it SHALL use CPU execution provider
- **AND** it SHALL enable all graph optimization levels
- **AND** it SHALL set intra-op num threads if specified

#### Scenario: Session options configuration

- **WHEN** configuring session options
- **THEN** graph optimization level SHALL be ORT_ENABLE_ALL
- **AND** execution mode SHALL be sequential
- **AND** log severity level SHALL be 3 (suppress warnings)
- **AND** it MAY set execution mode to parallel if beneficial

#### Scenario: Model file paths

- **WHEN** loading model files
- **THEN** encoder.int4.onnx SHALL be loaded from `{model_dir}/onnx_models/encoder.int4.onnx`
- **AND** decoder_init.int4.onnx SHALL be loaded from `{model_dir}/onnx_models/decoder_init.int4.onnx`
- **AND** decoder_step.int4.onnx SHALL be loaded from `{model_dir}/onnx_models/decoder_step.int4.onnx`

#### Scenario: Session creation failure

- **WHEN** model file is missing or corrupted
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL specify which model failed to load

### Requirement: Encoder Conv inference

The system SHALL run the encoder convolutional model to downsample Mel spectrogram.

> Note: this spec is now stale relative to the new `andrewleech/qwen3-asr-0.6b-onnx` export. The implementation uses `encoder.int4.onnx` with `decoder_init.int4.onnx` / `decoder_step.int4.onnx` and shared `decoder_weights.int4.data`.

#### Scenario: Input tensor format

- **WHEN** preparing input for encoder_conv
- **THEN** input name SHALL be `"padded_mel_chunks"`
- **AND** shape SHALL be `[chunk_num, 1, n_mels, max_chunk_len]` = `[N, 1, 128, L]`
- **AND** dtype SHALL be float32
- **AND** chunks SHALL be padded to equal length

#### Scenario: Chunking strategy

- **WHEN** Mel spectrogram has > 100 frames
- **THEN** it SHALL split into chunks of 100 frames each
- **AND** last chunk MAY be shorter
- **AND** it SHALL pad shorter chunks with zeros to match longest

#### Scenario: Output tensor

- **WHEN** running encoder_conv inference
- **THEN** output SHALL be shape `[chunk_num, max_out_len, hidden_dim]`
- **AND** hidden_dim SHALL be 896
- **AND** max_out_len SHALL be computed from input length via stride formula

#### Scenario: Remove padding

- **WHEN** conv inference completes
- **THEN** it SHALL remove padding from output chunks
- **AND** it SHALL compute output length for each chunk independently
- **AND** it SHALL concatenate chunks along sequence dimension

### Requirement: Encoder Transformer inference

The system SHALL run the encoder transformer model to process conv outputs.

#### Scenario: Input tensor format

- **WHEN** preparing input for encoder_transformer
- **THEN** input name SHALL be `"hidden_states"`
- **AND** shape SHALL be `[total_tokens, hidden_dim]` = `[N, 896]`
- **AND** dtype SHALL be float32
- **AND** total_tokens SHALL equal sum of conv output lengths

#### Scenario: Attention mask

- **WHEN** preparing attention mask for transformer
- **THEN** mask name SHALL be `"attention_mask"`
- **AND** shape SHALL be `[1, 1, total_tokens, total_tokens]`
- **AND** mask SHALL be all zeros (no masking needed for CPU all-to-all attention)

#### Scenario: Output tensor

- **WHEN** running encoder_transformer inference
- **THEN** output SHALL be shape `[total_tokens, projected_dim]` = `[N, 1024]`
- **AND** dtype SHALL be float32
- **AND** output SHALL be projected from 896 to 1024 dimensions

#### Scenario: Transformer layer details

- **WHEN** transformer runs
- **THEN** it SHALL use 18 transformer layers
- **AND** it SHALL use 14 attention heads
- **AND** hidden dimension SHALL be 896
- **AND** it SHALL use GQA (Grouped Query Attention) if applicable

### Requirement: Decoder Init inference (prefill)

The system SHALL run the decoder initialization model for the first forward pass.

#### Scenario: Input tensor format

- **WHEN** preparing input for decoder_init
- **THEN** input name SHALL be `"input_embeds"`
- **AND** shape SHALL be `[1, seq_len, hidden_dim]` = `[1, S, 1024]`
- **AND** dtype SHALL be float32
- **AND** seq_len SHALL equal total prompt length

#### Scenario: Position IDs

- **WHEN** preparing position_ids for decoder_init
- **THEN** shape SHALL be `[1, seq_len]` = `[1, S]`
- **AND** values SHALL be `[0, 1, 2, ..., seq_len-1]`
- **AND** dtype SHALL be int64

#### Scenario: Output logits

- **WHEN** running decoder_init inference
- **THEN** output logits SHALL have shape `[1, seq_len, vocab_size]` = `[1, S, 151936]`
- **AND** dtype SHALL be float32
- **AND** logits SHALL represent probability distribution over vocabulary

#### Scenario: KV cache output

- **WHEN** decoder_init runs with INT8 quantization
- **THEN** present_keys SHALL have shape `[num_layers, batch, kv_heads, seq_len, head_dim]`
- **AND** present_values SHALL have shape `[num_layers, batch, kv_heads, seq_len, head_dim]`
- **AND** num_layers SHALL be 28
- **AND** kv_heads SHALL be 8 (GQA: 8 key-value heads)
- **AND** head_dim SHALL be 128
- **AND** batch SHALL be 1
- **AND** dtype SHALL be float32 (even with INT8 weights, cache is FP32)

### Requirement: Decoder Step inference (autoregressive)

The system SHALL run the decoder step model for subsequent token generation.

#### Scenario: Input tensor format

- **WHEN** preparing input for decoder_step
- **THEN** input name SHALL be `"input_embeds"`
- **AND** shape SHALL be `[1, 1, hidden_dim]` = `[1, 1, 1024]` (single token)
- **AND** dtype SHALL be float32

#### Scenario: Position IDs

- **WHEN** preparing position_ids for decoder_step
- **THEN** shape SHALL be `[1, 1]`
- **AND** value SHALL be `[[cur_pos]]` where cur_pos is current sequence position
- **AND** dtype SHALL be int64

#### Scenario: Past KV cache input

- **WHEN** providing cached keys/values
- **THEN** past_keys name SHALL be `"past_keys"`
- **AND** past_values name SHALL be `"past_values"`
- **AND** shapes SHALL be `[num_layers, batch, kv_heads, cur_len, head_dim]`
- **AND** cur_len SHALL be current sequence length (excluding new token)

#### Scenario: Output logits

- **WHEN** running decoder_step inference
- **THEN** output logits SHALL have shape `[1, 1, vocab_size]` (only last position)
- **AND** this enables efficient greedy decoding without recomputing entire sequence

#### Scenario: Extended KV cache output

- **WHEN** decoder_step completes
- **THEN** present_keys SHALL have shape `[num_layers, batch, kv_heads, cur_len+1, head_dim]`
- **AND** present_values SHALL have shape `[num_layers, batch, kv_heads, cur_len+1, head_dim]`
- **AND** sequence length SHALL be incremented by 1

### Requirement: KV cache management

The system SHALL manage key-value cache across decoder steps for efficient autoregressive generation.

#### Scenario: Initialize KV cache

- **WHEN** decoder_init completes
- **THEN** system SHALL store present_keys and present_values
- **AND** it SHALL use them as past_keys and past_values for first decoder_step

#### Scenario: Update KV cache

- **WHEN** each decoder_step completes
- **THEN** system SHALL replace old cache with new present_keys/values
- **AND** old cache SHALL be dropped to free memory
- **AND** cache SHALL grow by 1 token per step

#### Scenario: Cache memory efficiency

- **WHEN** managing KV cache
- **THEN** it SHALL reuse allocations where possible
- **AND** it SHALL avoid unnecessary copies of cache tensors
- **AND** it MAY use in-place operations if safe

### Requirement: Greedy token decoding

The system SHALL implement greedy decoding to select most likely tokens at each step.

#### Scenario: Select next token

- **WHEN** decoder produces logits
- **THEN** system SHALL extract last logits vector: `logits[0, -1, :]`
- **AND** it SHALL compute argmax: `next_token = argmax(logits)`
- **AND** next_token SHALL be u32 scalar
- **AND** it SHALL NOT require copying the full logits tensor into a new owned array before argmax

#### Scenario: Append to sequence

- **WHEN** next token is selected
- **THEN** it SHALL append to generated tokens list
- **AND** it SHALL increment position counter
- **AND** it SHALL continue until stop condition

#### Scenario: Stop conditions

- **WHEN** checking stop conditions
- **THEN** it SHALL stop if token == IM_END_ID (151645)
- **AND** it SHALL stop if token == ENDOFTEXT_ID (151643)
- **AND** it SHALL stop if generated tokens reach max_new_tokens
- **AND** stop tokens SHALL NOT be included in final output

### Requirement: INT8 quantization support

The system SHALL support INT8-quantized decoder models for improved performance.

#### Scenario: Use INT8 models by default

- **WHEN** INT8 model files exist (decoder_init.int8.onnx, decoder_step.int8.onnx)
- **THEN** system SHALL load INT8 models instead of FP32
- **AND** inference SHALL use INT8 computation for weights
- **AND** KV cache SHALL remain FP32

#### Scenario: Fallback to FP32

- **WHEN** INT8 model files do not exist
- **THEN** system SHALL load INT4 models (decoder_init.int4.onnx, decoder_step.int4.onnx)
- **AND** it SHALL emit warning about using FP32 (slower)

#### Scenario: Performance characteristics

- **WHEN** using INT8 models
- **THEN** memory usage SHALL be approximately 50% of FP32
- **AND** speed SHALL be 2-3x faster than FP32
- **AND** accuracy SHALL be nearly identical (< 1% WER difference)

### Requirement: Multi-threading optimization

The system SHALL utilize multiple CPU cores for parallel computation.

#### Scenario: Intra-op parallelism

- **WHEN** running ONNX inference
- **THEN** it SHALL use all available CPU cores by default
- **AND** it MAY respect user-specified thread count
- **AND** each session SHALL have independent thread pool

#### Scenario: Inter-op parallelism

- **WHEN** running independent operations (e.g., multiple chunks)
- **THEN** it MAY use rayon for parallel processing
- **AND** it SHALL respect thread count limits to avoid oversubscription

### Requirement: Memory management

The system SHALL manage memory efficiently to handle long audio sessions.

#### Scenario: Tensor reuse

- **WHEN** running repeated inference
- **THEN** it SHALL reuse input/output tensors where possible
- **AND** it SHALL avoid allocating new tensors every iteration

#### Scenario: Memory cleanup

- **WHEN** processing audio chunks
- **THEN** it SHALL free chunk memory after processing
- **AND** it SHALL drop KV cache after transcription completes

#### Scenario: Peak memory estimation

- **WHEN** processing audio
- **THEN** peak memory SHALL be approximately:
  - Encoder: ~1GB for 60s audio
  - Decoder: ~4GB for KV cache (depends on sequence length)
  - Total: ~5-6GB peak with 512 max tokens

### Requirement: Error handling for ONNX operations

The system SHALL provide detailed error messages for ONNX Runtime failures.

#### Scenario: Invalid input shape

- **WHEN** input tensor has incorrect shape
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL specify expected vs actual shape
- **AND** it SHALL include model name and input tensor name

#### Scenario: OOM during inference

- **WHEN** ONNX Runtime runs out of memory
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL suggest enabling VAD chunking or reducing chunk size

#### Scenario: Session creation failure

- **WHEN** ONNX session creation fails
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL include underlying ONNX error message
- **AND** it MAY suggest checking model file integrity

#### Scenario: Type mismatch

- **WHEN** tensor dtype does not match model expectation
- **THEN** system SHALL return `InferenceError`
- **AND** error SHALL specify expected vs actual dtype

### Requirement: ONNX Runtime version compatibility

The system SHALL use compatible ONNX Runtime version via Nix and ort crate.

#### Scenario: System library dependency

- **WHEN** building the project
- **THEN** flake.nix SHALL include onnxruntime in buildInputs
- **AND** nixpkgs version SHALL provide ONNX Runtime 1.16+ or compatible

#### Scenario: Ort crate version

- **WHEN** specifying dependencies
- **THEN** ort crate SHALL be version 2.0 or compatible
- **AND** it SHALL use CPU-only features (no CUDA)

#### Scenario: Dynamic linking

- **WHEN** running the application
- **THEN** ort crate SHALL dynamically link to system ONNX Runtime
- **AND** library path SHALL be configured by Nix environment
