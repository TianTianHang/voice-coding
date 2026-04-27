#!/usr/bin/env bash
set -euo pipefail

SINK_NAME="VoiceCodingVirtualSink"
SOURCE_NAME="${SINK_NAME}.monitor"
STATE_FILE="/tmp/voice-coding-default-source"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: missing required command '$1'." >&2
    exit 1
  fi
}

check_audio_server() {
  if ! pactl info >/dev/null 2>&1; then
    echo "Error: could not connect to PipeWire/PulseAudio server." >&2
    exit 1
  fi
}

module_ids_for_sink() {
  pactl list modules short | awk -v sink="$SINK_NAME" '$2 == "module-null-sink" && $0 ~ ("sink_name=" sink) { print $1 }'
}

source_exists() {
  pactl list sources short | awk '{print $2}' | grep -Fx "$1" >/dev/null 2>&1
}

restore_default_source() {
  if [[ -f "$STATE_FILE" ]]; then
    local previous
    previous="$(cat "$STATE_FILE")"
    if [[ -n "$previous" ]] && source_exists "$previous"; then
      pactl set-default-source "$previous"
      echo "Restored default source to '$previous'."
    fi
    rm -f "$STATE_FILE"
  fi
}

main() {
  require_command pactl
  check_audio_server

  restore_default_source

  local ids
  ids="$(module_ids_for_sink || true)"
  if [[ -z "$ids" ]]; then
    echo "No module-null-sink found for '$SINK_NAME'. Nothing to clean."
    exit 0
  fi

  while read -r id; do
    [[ -z "$id" ]] && continue
    pactl unload-module "$id"
    echo "Unloaded module id $id for sink '$SINK_NAME'."
  done <<< "$ids"

  if source_exists "$SOURCE_NAME"; then
    echo "Warning: source '$SOURCE_NAME' still exists." >&2
    exit 1
  fi

  echo "Virtual audio cleanup complete."
}

main "$@"
