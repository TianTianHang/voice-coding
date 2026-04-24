import { useRef } from "react";
import { useBackendVAD } from "../hooks/useBackendVAD";
import { AudioVisualizer } from "./AudioVisualizer";
import { TranscriptDisplay } from "./TranscriptDisplay";
import { ControlButton } from "./ControlButton";

export function VoiceRecorder() {
  const transcriptHistoryRef = useRef<string[]>([]);

  const { state, transcript, error, recordingDuration, startListening, stopListening } =
    useBackendVAD();

  if (transcript && transcript !== transcriptHistoryRef.current[transcriptHistoryRef.current.length - 1]) {
    transcriptHistoryRef.current = [...transcriptHistoryRef.current, transcript];
  }

  const displayText = transcript || transcriptHistoryRef.current.join("\n");

  const errorGuidance = error
    ? error.includes("denied") || error.includes("NotAllowedError")
      ? `${error} — Please grant microphone permission in your system settings and try again.`
      : error
    : null;

  return (
    <div style={{ maxWidth: 600, margin: "0 auto", padding: 20 }}>
      <h1 style={{ textAlign: "center", marginBottom: 24 }}>Voice Coding</h1>
      <p style={{ textAlign: "center", color: "#888", fontSize: 14, marginBottom: 16 }}>
        Click "Start Listening", then speak. The app will automatically detect your voice and transcribe it.
      </p>

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
          Transcribing audio...
        </div>
      )}

      {errorGuidance && (
        <div style={{ padding: 12, backgroundColor: "#ffeaea", borderRadius: 8, color: "#c0392b", marginBottom: 12 }}>
          {errorGuidance}
        </div>
      )}

      <TranscriptDisplay text={displayText} error={null} />
    </div>
  );
}
