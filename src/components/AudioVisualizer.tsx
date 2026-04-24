import type { CSSProperties } from "react";
import type { VADState } from "../hooks/useVAD";

interface AudioVisualizerProps {
  state: VADState;
  recordingDuration: number;
}

const stateConfig: Record<VADState, { label: string; color: string }> = {
  idle: { label: "Ready", color: "#888" },
  listening: { label: "Listening...", color: "#4a90d9" },
  recording: { label: "Recording...", color: "#e74c3c" },
  processing: { label: "Processing...", color: "#f39c12" },
};

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
