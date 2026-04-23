# ONNX Models Input/Output Specification Report
This document describes the input and output specifications for all ONNX models
used in the voice-coding project.

## decoder_init.int8.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `input_embeds` | FLOAT | [batch, seq_len, 1024] |
| 1 | `position_ids` | INT64 | [batch, seq_len] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `logits` | FLOAT | [batch, seq_len, 151936] |
| 1 | `present_keys` | FLOAT | [28, batch, 8, seq_len, 128] |
| 2 | `present_values` | FLOAT | [28, batch, 8, seq_len, 128] |

---

## decoder_step.int8.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `input_embeds` | FLOAT | [batch, 1, 1024] |
| 1 | `position_ids` | INT64 | [batch, 1] |
| 2 | `past_keys` | FLOAT | [28, batch, 8, past_seq_len, 128] |
| 3 | `past_values` | FLOAT | [28, batch, 8, past_seq_len, 128] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `logits` | FLOAT | [batch, 1, 151936] |
| 1 | `present_keys` | FLOAT | [28, batch, 8, past_seq_len + 1, 128] |
| 2 | `present_values` | FLOAT | [28, batch, 8, past_seq_len + 1, 128] |

---

## encoder_conv.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `padded_mel_chunks` | FLOAT | [num_chunks, 1, 128, chunk_len] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `chunk_features` | FLOAT | [num_chunks, (((chunk_len - 1)//8)) + 1, 896] |

---

## encoder_transformer.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `hidden_states` | FLOAT | [total_tokens, 896] |
| 1 | `attention_mask` | FLOAT | [1, 1, total_tokens, total_tokens] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `encoder_output` | FLOAT | [total_tokens, 1024] |

---

## Comparison with Rust Implementation

### encoder_conv.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/encoder.rs:81`
- **Input name**: `padded_mel_chunks`
- **Expected input**: 4D tensor `[batch, n_chunks, n_mels, chunk_len]`

### encoder_transformer.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/encoder.rs:150-156`
- **Input**: Output from encoder_conv
- **Outputs**: Encoder representations for decoder

### decoder_init.int8.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:84-90`
- **Input name**: `input_embeds`
- **Expected inputs**: 
  - `input_embeds`: 3D tensor `[1, seq_len, hidden_size]`
- **Outputs**: 
  - `logits`: Token prediction logits
  - `present_keys`: KV-cache keys
  - `present_values`: KV-cache values

### decoder_step.int8.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:216-224`
- **Expected inputs**:
  - `input_embeds`: Single token embedding
  - KV-cache from previous steps
- **Outputs**: 
  - `logits`: Token prediction logits
  - Updated KV-cache
