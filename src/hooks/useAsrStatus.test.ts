import { describe, expect, it } from "vitest";
import {
  asrStatusLabel,
  initialAsrStatus,
  replaceAsrStatusFromEvent,
  type AsrStatusSnapshot,
  type ModelPathSnapshot,
} from "./useAsrStatus";

const asrModel: ModelPathSnapshot = {
  kind: "asr",
  modelId: "qwen3-asr-0.6b-onnx",
  engineName: "qwen3-asr-0.6b",
  packageDir: "models/asr/qwen3-asr-0.6b-onnx",
  modelDir: "models/asr/qwen3-asr-0.6b-onnx",
  source: "devFallback",
  legacyLayout: false,
  missingFiles: [],
};

describe("asrStatusLabel", () => {
  it("maps ready snapshots with timing directly from the latest payload", () => {
    const status: AsrStatusSnapshot = {
      state: "ready",
      engineName: "qwen3-asr-0.6b",
      modelDir: "models",
      model: asrModel,
      timing: {
        totalMs: 1200,
        onnxSessionsMs: 600,
        embeddingsMs: 400,
        tokenizerMs: 100,
        melFilterbankMs: 10,
      },
    };

    expect(asrStatusLabel(status)).toBe("Speech model ready in 1200 ms");
  });

  it("maps failed snapshots with the backend error message", () => {
    const status: AsrStatusSnapshot = {
      state: "failed",
      engineName: "qwen3-asr-0.6b",
      modelDir: "models",
      model: { ...asrModel, error: "Embedding file not found" },
      error: "Embedding file not found",
    };

    expect(asrStatusLabel(status)).toBe(
      "Speech model failed: Embedding file not found",
    );
  });
});

describe("replaceAsrStatusFromEvent", () => {
  it("replaces local state with the latest full snapshot payload", () => {
    const current: AsrStatusSnapshot = {
      state: "ready",
      engineName: "qwen3-asr-0.6b",
      modelDir: "models",
      model: asrModel,
      timing: {
        totalMs: 900,
        onnxSessionsMs: 400,
        embeddingsMs: 320,
        tokenizerMs: 120,
        melFilterbankMs: 60,
      },
    };
    const incoming: AsrStatusSnapshot = {
      state: "loading",
      engineName: "qwen3-asr-0.6b",
      modelDir: "models",
      model: { ...asrModel, source: "modelHomeEnv" },
      phase: "model",
    };

    expect(replaceAsrStatusFromEvent(current, incoming)).toEqual(incoming);
  });

  it("keeps model diagnostics as part of the initial full snapshot", () => {
    expect(initialAsrStatus.model).toMatchObject({
      kind: "asr",
      modelId: "qwen3-asr-0.6b-onnx",
      modelDir: "",
      missingFiles: [],
    });
  });
});
