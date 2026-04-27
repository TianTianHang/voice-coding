#!/usr/bin/env bash
set -euo pipefail

SINK_NAME="VoiceCodingVirtualSink"
SOURCE_NAME="${SINK_NAME}.monitor"
STATE_FILE="/tmp/voice-coding-default-source"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: missing required command '$1'." >&2
    echo "Hint: enter the flake dev shell first (nix develop)." >&2
    exit 1
  fi
}

check_audio_server() {
  if ! pactl info >/dev/null 2>&1; then
    echo "Error: could not connect to PipeWire/PulseAudio server." >&2
    echo "Hint: ensure pipewire-pulse is running for your user session." >&2
    exit 1
  fi
}

get_current_default_source() {
  pactl info | sed -n 's/^Default Source: //p'
}

find_module_id() {
  pactl list modules short | awk -v sink="$SINK_NAME" '$2 == "module-null-sink" && $0 ~ ("sink_name=" sink) { print $1; exit }'
}

source_exists() {
  pactl list sources short | awk '{print $2}' | grep -Fx "$SOURCE_NAME" >/dev/null 2>&1
}

main() {
  require_command pactl
  check_audio_server

  local default_before
  default_before="$(get_current_default_source)"

  local module_id
  module_id="$(find_module_id || true)"

  if [[ -z "$module_id" ]]; then
    module_id="$(pactl load-module module-null-sink \
      sink_name="$SINK_NAME" \
      sink_properties=device.description="VoiceCoding_Virtual_Sink" \
      rate=16000 \
      channels=1 \
      format=s16le)"
    echo "Created virtual sink '$SINK_NAME' (module id: $module_id)."
  else
    echo "Virtual sink '$SINK_NAME' already exists (module id: $module_id)."
  fi

  if ! source_exists; then
    echo "Error: expected source '$SOURCE_NAME' was not found after setup." >&2
    exit 1
  fi

  if [[ -n "$default_before" && "$default_before" != "$SOURCE_NAME" ]]; then
    printf '%s\n' "$default_before" > "$STATE_FILE"
  fi

  pactl set-default-source "$SOURCE_NAME"

  echo ""
  echo "Virtual input is ready for Rust backend recording:"
  echo "  Default source: $SOURCE_NAME"
  echo ""
  echo "Use one of these ways to feed audio into it:"
  echo "  1) In desktop sound settings, set app output device to '$SINK_NAME'"
  echo "  2) CLI example: PULSE_SINK=$SINK_NAME paplay /path/to/file.wav"
  echo ""
  echo "Inspect devices:"
  echo "  pactl list sinks short"
  echo "  pactl list sources short"
  echo ""
  echo "Cleanup when done: bash scripts/cleanup_virtual_audio.sh"
}

main "$@"
