import { describe, expect, it } from "vitest";
import {
  asrStatusLabel,
  replaceAsrStatusFromEvent,
  type AsrStatusSnapshot,
} from "./useAsrStatus";

describe("asrStatusLabel", () => {
  it("maps ready snapshots with timing directly from the latest payload", () => {
    const status: AsrStatusSnapshot = {
      state: "ready",
      engineName: "qwen3-asr-0.6b",
      modelDir: "models",
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
      phase: "model",
    };

    expect(replaceAsrStatusFromEvent(current, incoming)).toEqual(incoming);
  });
});
