import type { VADState } from "../hooks/useBackendVAD";

interface ControlButtonProps {
  state: VADState;
  onStart: () => void;
  onStop: () => void;
}

export function ControlButton({ state, onStart, onStop }: ControlButtonProps) {
  const isIdle = state === "idle";
  const isProcessing = state === "processing";

  return (
    <button
      onClick={isIdle ? onStart : onStop}
      disabled={isProcessing}
      style={{
        padding: "12px 32px",
        fontSize: 16,
        fontWeight: 600,
        borderRadius: 8,
        border: "none",
        cursor: isProcessing ? "not-allowed" : "pointer",
        backgroundColor: isIdle ? "#4a90d9" : "#e74c3c",
        color: "#fff",
        opacity: isProcessing ? 0.6 : 1,
        transition: "background-color 0.2s, opacity 0.2s",
      }}
    >
      {isProcessing
        ? "Processing current utterance..."
        : isIdle
          ? "Start Listening"
          : "Stop Session"}
    </button>
  );
}
