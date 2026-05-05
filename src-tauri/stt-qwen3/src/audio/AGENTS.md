# AGENTS.md

Audio preprocessing for Qwen3 ASR.

## Module Map
- `loader.rs` loads audio from files/bytes, validates samples, and resamples to the model sample rate.
- `mel.rs` computes mel spectrogram features and filterbanks expected by the encoder.
- `vad.rs` finds split points and chunks long audio before inference.

## Relationships
- Called by `stt-qwen3/src/lib.rs` before encoder inference.
- Must align with `openspec/specs/audio-preprocessing/spec.md`.

## Editing Notes
- Preserve 16 kHz model input expectations unless updating all callers and docs.
- Add boundary tests when changing chunking, validation, or resampling behavior.
