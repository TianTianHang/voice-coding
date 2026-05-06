import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AsrLoadState = "unloaded" | "loading" | "ready" | "failed";

export type AsrLoadTiming = {
  totalMs: number;
  onnxSessionsMs: number;
  embeddingsMs: number;
  tokenizerMs: number;
  melFilterbankMs: number;
};

export type ModelKind = "asr" | "tts";

export type ModelPathSource =
  | "engineEnv"
  | "modelHomeEnv"
  | "appData"
  | "devFallback"
  | "legacyDevFallback";

export type MissingModelFile = {
  path: string;
  description: string;
};

export type ModelPathSnapshot = {
  kind: ModelKind;
  modelId: string;
  engineName: string;
  packageDir: string;
  modelDir: string;
  source: ModelPathSource;
  legacyLayout: boolean;
  missingFiles: MissingModelFile[];
  error?: string;
};

export type AsrStatusSnapshot = {
  state: AsrLoadState;
  engineName: string;
  modelDir: string;
  model: ModelPathSnapshot;
  phase?: string;
  timing?: AsrLoadTiming;
  error?: string;
};

export interface AsrStatusResult {
  status: AsrStatusSnapshot;
  error: string | null;
}

export const initialAsrStatus: AsrStatusSnapshot = {
  state: "unloaded",
  engineName: "qwen3-asr-0.6b",
  modelDir: "",
  model: {
    kind: "asr",
    modelId: "qwen3-asr-0.6b-onnx",
    engineName: "qwen3-asr-0.6b",
    packageDir: "",
    modelDir: "",
    source: "devFallback",
    legacyLayout: false,
    missingFiles: [],
  },
};

export function replaceAsrStatusFromEvent(
  _current: AsrStatusSnapshot,
  incoming: AsrStatusSnapshot,
): AsrStatusSnapshot {
  return incoming;
}

export function asrStatusLabel(status: AsrStatusSnapshot): string {
  switch (status.state) {
    case "loading":
      return "Preparing speech model";
    case "ready":
      return status.timing
        ? `Speech model ready in ${status.timing.totalMs} ms`
        : "Speech model ready";
    case "failed":
      return status.error
        ? `Speech model failed: ${status.error}`
        : "Speech model failed";
    case "unloaded":
      return "Speech model not loaded";
  }
}

export function useAsrStatus(): AsrStatusResult {
  const [status, setStatus] = useState<AsrStatusSnapshot>(initialAsrStatus);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    async function setup() {
      const current = await invoke<AsrStatusSnapshot>("get_asr_status");
      if (!disposed) {
        setStatus(current);
      }

      unlisten = await listen<AsrStatusSnapshot>("asr-status", (event) => {
        setStatus((current) => replaceAsrStatusFromEvent(current, event.payload));
        setError(null);
      });

      const prepared = await invoke<AsrStatusSnapshot>("prepare_asr");
      if (!disposed) {
        setStatus(prepared);
      }
    }

    setup().catch((e) => {
      if (!disposed) {
        setError(String(e));
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return { status, error };
}
