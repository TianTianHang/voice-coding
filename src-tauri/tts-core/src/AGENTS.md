# AGENTS.md

Source for the engine-agnostic TTS contract crate.

## Folder Role
- `lib.rs` defines public TTS traits, errors, configs, PCM/audio buffers, synthesis events, and playback validation.
- Concrete engines such as `src-tauri/tts-moss/` implement these traits.

## Relationships
- `src-tauri/src/tts.rs` stores a `dyn TtsEngine` and exposes Tauri commands around these types.
- `src-tauri/src/audio/output.rs` consumes `AudioBuffer` and relies on the playback constants defined here.

## Editing Notes
- Keep this crate free of Tauri and engine-specific dependencies.
- Changing sample-rate/channel requirements requires updating playback code, engines, docs, and tests.
