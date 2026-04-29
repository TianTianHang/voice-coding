import type { VADState } from "../hooks/useBackendVAD";

interface ControlButtonProps {
  state: VADState;
  onStart: () => void;
  onStop: () => void;
}

export function ControlButton({ state, onStart, onStop }: ControlButtonProps) {
  const isIdle = state === "idle";
  const isProcessing = state === "processing";
  const buttonClass = isIdle
    ? "bg-emerald-600 text-white hover:bg-emerald-700"
    : isProcessing
      ? "bg-slate-100 text-slate-700"
      : "bg-rose-600 text-white hover:bg-rose-700";

  return (
    <button
      className={`min-h-10 min-w-[150px] cursor-pointer rounded-lg px-4 text-sm font-extrabold transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60 ${buttonClass}`}
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
