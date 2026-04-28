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
      className={`primary-button ${isIdle ? "start" : isProcessing ? "processing" : "stop"}`}
      onClick={isIdle ? onStart : onStop}
      disabled={isProcessing}
    >
      {isProcessing
        ? "Processing"
        : isIdle
          ? "Start Listening"
          : "Stop Session"}
    </button>
  );
}
