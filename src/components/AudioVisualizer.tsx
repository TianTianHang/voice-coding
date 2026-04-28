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
  const isActive = state !== "idle";

  const durationText =
    state === "recording" && recordingDuration > 0
      ? ` (${recordingDuration.toFixed(1)}s)`
      : "";

  return (
    <div className="flex items-center gap-2 py-3">
      <div
        className={`h-3 w-3 rounded-full ${
          state === "listening"
            ? "bg-emerald-600"
            : state === "recording"
              ? "bg-rose-600"
              : state === "processing"
                ? "bg-amber-600"
                : "bg-slate-500"
        } ${isActive ? "animate-pulse" : ""}`}
      />
      <span
        className={`text-base font-semibold ${
          state === "listening"
            ? "text-emerald-700"
            : state === "recording"
              ? "text-rose-700"
              : state === "processing"
                ? "text-amber-700"
                : "text-slate-600"
        }`}
      >
        {config.label}{durationText}
      </span>
    </div>
  );
}
