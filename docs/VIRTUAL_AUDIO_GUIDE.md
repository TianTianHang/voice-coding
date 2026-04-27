# Virtual Audio Input Guide

This guide sets up a virtual microphone for the Rust backend recorder in this project.

## Why this works

- The backend recorder uses `cpal::default_input_device()`.
- With PipeWire + PulseAudio compatibility, a `module-null-sink` creates a monitor source.
- That monitor source can be selected as the system default input device.
- Once set as default, the Rust recorder receives audio routed into the virtual sink.

## Prerequisites

- Run inside this repository's flake dev shell.
- User session has PipeWire/PulseAudio compatibility enabled (`pipewire-pulse`).

## Quick start

1. Enter dev shell.
2. Create the virtual input:

```bash
setup-virtual-audio
```

3. Route playback audio into `VoiceCodingVirtualSink`.
4. Start app recording (`pnpm tauri dev`) and test VAD/transcription.

## Commands

- Setup virtual input:

```bash
bash scripts/setup_virtual_audio.sh
```

- Cleanup virtual input:

```bash
bash scripts/cleanup_virtual_audio.sh
```

- Inspect devices:

```bash
pactl list sinks short
pactl list sources short
```

## Feed audio to virtual input

Example using `paplay`:

```bash
PULSE_SINK=VoiceCodingVirtualSink paplay /path/to/test.wav
```

Or choose `VoiceCodingVirtualSink` as output device for a desktop app in your sound settings.

## Troubleshooting

- `pactl info` fails:
  - PipeWire/PulseAudio server is not reachable in current session.
  - Ensure `pipewire`, `pipewire-pulse`, and `wireplumber` are running for your user.

- Virtual source not visible:
  - Run `pactl list modules short` and confirm a `module-null-sink` entry exists.
  - Re-run setup script.

- Backend still records physical microphone:
  - Check default source: `pactl info | grep "Default Source"`.
  - Ensure it is `VoiceCodingVirtualSink.monitor`.

- Need to restore previous default device:
  - Run cleanup script; it restores previously saved default source when available.
