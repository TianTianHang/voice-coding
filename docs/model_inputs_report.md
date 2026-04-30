# ONNX Models Input/Output Specification Report
This document describes the input and output specifications for all ONNX models
used in the voice-coding project.

## decoder_init.int4.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `input_ids` | INT64 | [batch, seq_len] |
| 1 | `position_ids` | INT64 | [batch, seq_len] |
| 2 | `audio_features` | FLOAT | [1, audio_len, 1024] |
| 3 | `audio_offset` | INT64 | [1, 1] |
| 1 | `position_ids` | INT64 | [batch, seq_len] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `logits` | FLOAT | [batch, seq_len, 151936] |
| 1 | `present_keys` | FLOAT | [28, batch, 8, seq_len, 128] |
| 2 | `present_values` | FLOAT | [28, batch, 8, seq_len, 128] |

---

## decoder_step.int4.onnx

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

## encoder.int4.onnx

**Graph Name**: `main_graph`

### Inputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `mel` | FLOAT | [1, 128, frames] |

### Outputs

| Index | Name | Type | Shape |
|-------|------|------|-------|
| 0 | `audio_features` | FLOAT | [1, tokens, 1024] |

---

## Comparison with Rust Implementation

### encoder.int4.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/encoder.rs:1`
- **Input name**: `mel`
- **Expected input**: 3D tensor `[1, 128, frames]`

### decoder_init.int4.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:53`
- **Expected inputs**:
  - `input_ids`: prompt token IDs
  - `position_ids`: absolute positions
  - `audio_features`: encoder output inserted for audio placeholders
  - `audio_offset`: audio placeholder start index
- **Outputs**:
  - `logits`: Token prediction logits
  - `present_keys`: KV-cache keys
  - `present_values`: KV-cache values

### decoder_step.int4.onnx
- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:208-216`
- **Expected inputs**:
  - `input_embeds`: Single token embedding
  - KV-cache from previous steps
- **Outputs**:
  - `logits`: Token prediction logits
  - Updated KV-cache
