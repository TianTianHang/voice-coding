import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type VADState = "idle" | "listening" | "recording" | "processing";

export interface BackendVADResult {
  state: VADState;
  transcript: string;
  error: string | null;
  recordingDuration: number;
  startListening: () => Promise<void>;
  stopListening: () => Promise<void>;
}

export function useBackendVAD(): BackendVADResult {
  const [state, setState] = useState<VADState>("idle");
  const [transcript, setTranscript] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [recordingDuration, setRecordingDuration] = useState(0);
  const durationRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    async function setup() {
      unlisteners.push(
        await listen<{ state: string }>("vad-state", (event) => {
          const newState = event.payload.state as VADState;
          setState(newState);

          if (newState === "recording") {
            const start = Date.now();
            if (durationRef.current) clearInterval(durationRef.current);
            durationRef.current = setInterval(() => {
              setRecordingDuration((Date.now() - start) / 1000);
            }, 100);
          } else {
            if (durationRef.current) {
              clearInterval(durationRef.current);
              durationRef.current = null;
            }
            setRecordingDuration(0);
          }
        })
      );
      unlisteners.push(
        await listen<{ text: string }>("transcript", (event) => {
          setTranscript(event.payload.text);
        })
      );
      unlisteners.push(
        await listen<string>("error", (event) => {
          setError(event.payload);
        })
      );
    }

    setup();

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
      if (durationRef.current) {
        clearInterval(durationRef.current);
        durationRef.current = null;
      }
    };
  }, []);

  const startListening = useCallback(async () => {
    setError(null);
    try {
      await invoke("start_listening");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const stopListening = useCallback(async () => {
    try {
      await invoke("stop_listening");
    } catch (e) {
      setError(String(e));
    }
    setState("idle");
    setRecordingDuration(0);
    if (durationRef.current) {
      clearInterval(durationRef.current);
      durationRef.current = null;
    }
  }, []);

  return { state, transcript, error, recordingDuration, startListening, stopListening };
}
