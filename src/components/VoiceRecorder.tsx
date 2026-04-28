import { useBackendVAD } from "../hooks/useBackendVAD";
import { asrStatusLabel, useAsrStatus } from "../hooks/useAsrStatus";
import { AudioVisualizer } from "./AudioVisualizer";
import { TranscriptDisplay } from "./TranscriptDisplay";
import { ControlButton } from "./ControlButton";

export function VoiceRecorder() {
  const {
    state,
    transcript,
    error,
    recordingDuration,
    startListening,
    stopListening,
  } = useBackendVAD();
  const { status: asrStatus, error: asrStatusError } = useAsrStatus();

  const errorGuidance = error
    ? error.includes("denied") || error.includes("NotAllowedError")
      ? `${error} — Please grant microphone permission in your system settings and try again.`
      : error
    : asrStatusError;
  const asrStatusColor =
    asrStatus.state === "ready"
      ? "#2f855a"
      : asrStatus.state === "failed"
        ? "#c0392b"
        : "#8a6d1d";

  return (
    <div style={{ maxWidth: 600, margin: "0 auto", padding: 20 }}>
      <h1 style={{ textAlign: "center", marginBottom: 24 }}>Voice Coding</h1>
      <p style={{ textAlign: "center", color: "#888", fontSize: 14, marginBottom: 16 }}>
        Click "Start Listening", then speak. The session stays active after each result so you can
        continue with the next utterance.
      </p>
      <div
        style={{
          textAlign: "center",
          color: asrStatusColor,
          fontSize: 13,
          fontWeight: 600,
          marginBottom: 16,
        }}
      >
        {asrStatusLabel(asrStatus)}
      </div>

      <div style={{ display: "flex", justifyContent: "center", marginBottom: 16 }}>
        <ControlButton
          state={state}
          onStart={startListening}
          onStop={stopListening}
        />
      </div>

      <div style={{ display: "flex", justifyContent: "center" }}>
        <AudioVisualizer state={state} recordingDuration={recordingDuration} />
      </div>

      {state === "processing" && (
        <div style={{ textAlign: "center", padding: 8, color: "#f39c12" }}>
          Transcribing current utterance. Listening will resume automatically.
        </div>
      )}

      <TranscriptDisplay text={transcript} error={errorGuidance} />
    </div>
  );
}
