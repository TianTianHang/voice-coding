import type { CSSProperties } from "react";
import type { VADState } from "../hooks/useBackendVAD";

interface AudioVisualizerProps {
  state: VADState;
  recordingDuration: number;
}

const stateConfig: Record<VADState, { label: string; color: string }> = {
  idle: { label: "Stopped", color: "#888" },
  listening: { label: "Listening (waiting for speech)...", color: "#1f8a70" },
  recording: { label: "Recording", color: "#c23b45" },
  processing: { label: "Processing...", color: "#b06a00" },
};

export function getVadStatusLabel(state: VADState): string {
  return stateConfig[state].label;
}

export function AudioVisualizer({ state, recordingDuration }: AudioVisualizerProps) {
  const config = stateConfig[state];

  const dotStyle: CSSProperties = {
    width: 12,
    height: 12,
    borderRadius: "50%",
    backgroundColor: config.color,
    animation: state !== "idle" ? "pulse 1.5s ease-in-out infinite" : "none",
  };

  const durationText =
    state === "recording" && recordingDuration > 0
      ? ` (${recordingDuration.toFixed(1)}s)`
      : "";

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "12px 0" }}>
      <div style={dotStyle} />
      <span style={{ color: config.color, fontWeight: 600, fontSize: 16 }}>
        {config.label}{durationText}
      </span>
      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; transform: scale(1); }
          50% { opacity: 0.5; transform: scale(1.3); }
        }
      `}</style>
    </div>
  );
}
