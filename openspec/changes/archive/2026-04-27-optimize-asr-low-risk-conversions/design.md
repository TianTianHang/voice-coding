## Context

The current ASR path already supports `AudioInput::Samples`, but the real-time VAD integration still converts recorded 16kHz mono `i16` samples into WAV bytes and then decodes those bytes back into float32 samples. This is useful for generic byte-buffer input, but it is unnecessary for audio captured by the backend VAD path because the sample format is already known.

The decoder path also copies logits into owned arrays before greedy argmax. This keeps the code simple but allocates and copies the full vocabulary-sized logits buffer even though greedy decoding only needs to scan the extracted tensor data.

This change targets low-risk conversion overhead only. It deliberately avoids changing model files, ONNX graph structure, mel numerical precision, or KV cache ownership.

## Goals / Non-Goals

**Goals:**

- Remove the WAV encode/decode loop from backend VAD transcription.
- Keep external byte-buffer transcription behavior intact.
- Avoid owned logits array allocation for greedy token selection.
- Reduce small repeated scans in prompt/audio embedding fusion.
- Preserve transcript text, stop-token behavior, session filtering, and error handling semantics.

**Non-Goals:**

- Convert Mel/STFT computation from `f64` to `f32`.
- Redesign encoder tensors or replace `Vec<Vec<_>>` representations.
- Rework decoder KV cache ownership or ONNX I/O binding.
- Add new frontend controls or user-visible behavior.
- Add new model dependencies or execution providers.

## Decisions

1. VAD transcription SHALL pass raw samples to the STT engine.

   The VAD state machine emits `Vec<i16>` recorded at the backend VAD sample rate. The Tauri command layer should convert this buffer directly to `Vec<f32>` using the existing PCM normalization rule and call `AudioInput::Samples(samples, SAMPLE_RATE)`.

   Alternative considered: keep `AudioInput::Bytes` and implement a custom fast WAV parser. That still retains an unnecessary container format for a path with known sample metadata.

2. Byte-buffer transcription SHALL remain generic.

   `transcribe_audio_data(Vec<u8>, language)` and file-path transcription still need Symphonia because callers may provide encoded audio formats. This change only removes the real-time VAD path's artificial WAV construction.

   Alternative considered: specialize all WAV bytes with a fast path. That is useful later, but it broadens scope and requires more format validation.

3. Greedy decoding SHALL scan extracted logits data directly.

   The ONNX output extraction already provides tensor shape and data. Greedy argmax can validate the shape and scan the last vocabulary slice without constructing an `Array3<f32>`.

   Alternative considered: keep `Array3` conversion for readability. That preserves current behavior but continues copying a large vocabulary buffer for every decoder step.

4. Embedding fusion SHALL use a monotonic audio index.

   Prompt IDs contain audio pad tokens in sequence order. Instead of collecting all audio pad positions and searching for each position again, fusion can increment an `audio_idx` whenever it sees an audio pad token.

   Alternative considered: keep the current two-pass implementation. It is correct but does avoidable repeated scanning.

## Risks / Trade-offs

- Direct VAD sample conversion could introduce scale differences if the WAV decoder currently normalizes `i16::MIN` differently -> use the same `sample as f32 / 32768.0` convention already used by the loader.
- VAD path may lose generic format validation that Symphonia provided -> the VAD path has fixed sample metadata, so validation should focus on sample rate, duration, and non-empty buffers.
- Direct logits scanning may mis-index non-standard output shapes -> validate expected rank and use the final sequence position exactly as current greedy decoding does.
- Small fusion changes could mismatch audio token count -> keep the existing count validation and tests for mismatch behavior.
