import { useCallback, useRef } from "react";
import { useVAD } from "../hooks/useVAD";
import { encodeWAV } from "../hooks/useAudioRecorder";
import { useTranscription } from "../hooks/useTranscription";
import { AudioVisualizer } from "./AudioVisualizer";
import { TranscriptDisplay } from "./TranscriptDisplay";
import { ControlButton } from "./ControlButton";
import type { VADState } from "../hooks/useVAD";

const MIN_RECORDING_SAMPLES = 8000;

export function VoiceRecorder() {
  const transcriptHistoryRef = useRef<string[]>([]);
  const lastStateRef = useRef<VADState>("idle");

  const {
    text: transcriptText,
    isLoading,
    error: transcriptError,
    transcribe,
    reset: resetTranscription,
  } = useTranscription();

  const handleStateChange = useCallback((newState: VADState) => {
    if (newState === "idle" && lastStateRef.current === "processing") {
      resetTranscription();
    }
    lastStateRef.current = newState;
  }, [resetTranscription]);

  const handleRecordingStop = useCallback(
    async (audioData: Int16Array) => {
      if (audioData.length < MIN_RECORDING_SAMPLES) {
        resetTranscription();
        return;
      }

      const wavData = encodeWAV(audioData, 16000);
      const result = await transcribe(wavData);
      if (result) {
        transcriptHistoryRef.current = [
          ...transcriptHistoryRef.current,
          result,
        ];
      }
    },
    [transcribe, resetTranscription]
  );

  const handleRecordingStart = useCallback(() => {
    resetTranscription();
  }, [resetTranscription]);

  const handleError = useCallback((error: string) => {
    console.error("VAD Error:", error);
  }, []);

  const { state, error: vadError, startListening, stopListening, recordingDuration } = useVAD({
    onRecordingStart: handleRecordingStart,
    onRecordingStop: handleRecordingStop,
    onStateChange: handleStateChange,
    onError: handleError,
  });

  const displayError = vadError || (state === "idle" && lastStateRef.current !== "idle" ? null : null);
  const displayText = transcriptText || transcriptHistoryRef.current.join("\n");

  const errorGuidance = displayError
    ? displayError.includes("denied") || displayError.includes("NotAllowedError")
      ? `${displayError} — Please grant microphone permission in your system settings and try again.`
      : displayError
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

      {isLoading && (
        <div style={{ textAlign: "center", padding: 8, color: "#f39c12" }}>
          Transcribing audio...
        </div>
      )}

      {errorGuidance && (
        <div style={{ padding: 12, backgroundColor: "#ffeaea", borderRadius: 8, color: "#c0392b", marginBottom: 12 }}>
          {errorGuidance}
        </div>
      )}

      <TranscriptDisplay text={displayText} error={transcriptError} />
    </div>
  );
}
