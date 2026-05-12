import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface TranscriptionResult {
  text: string;
  isLoading: boolean;
  error: string | null;
  transcribe: (audioData: Uint8Array, language?: string) => Promise<string | null>;
  reset: () => void;
}

export function useTranscription(): TranscriptionResult {
  const [text, setText] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const transcribe = useCallback(
    async (audioData: Uint8Array, language?: string): Promise<string | null> => {
      setIsLoading(true);
      setError(null);

      try {
        const result = await invoke<string>("debug_transcribe_audio_data", {
          audioData: Array.from(audioData),
          language: language ?? null,
        });

        setText(result);
        setIsLoading(false);
        return result;
      } catch (err) {
        const message =
          err instanceof Error ? err.message : String(err);
        setError(message);
        setIsLoading(false);
        return null;
      }
    },
    []
  );

  const reset = useCallback(() => {
    setText("");
    setError(null);
    setIsLoading(false);
  }, []);

  return { text, isLoading, error, transcribe, reset };
}
