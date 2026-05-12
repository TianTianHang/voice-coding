import { describe, expect, it } from "vitest";
import {
  buildDebugStreamingAsrRequest,
  buildTtsInvokeConfig,
  formatAsrDebugTime,
  formatTtsDebugTime,
} from "./DebugToolsWindow";

const defaultTtsInput = {
  voice: "",
  samplingMode: "fixed" as const,
  referenceAudioPath: "",
  seed: "",
  maxNewFrames: "",
  textTemperature: "",
  textTopP: "",
  textTopK: "",
  audioTemperature: "",
  audioTopP: "",
  audioTopK: "",
  audioRepetitionPenalty: "",
};

describe("buildTtsInvokeConfig", () => {
  it("maps the default debug selection to fixed MOSS sampling", () => {
    expect(buildTtsInvokeConfig(defaultTtsInput)).toEqual({
      moss: { samplingMode: "fixed" },
    });
  });

  it("maps greedy sampling and trims the reference audio path", () => {
    expect(
      buildTtsInvokeConfig({
        ...defaultTtsInput,
        samplingMode: "greedy",
        referenceAudioPath: "  /tmp/ref.wav  ",
      }),
    ).toEqual({
      moss: {
        samplingMode: "greedy",
        referenceAudioPath: "/tmp/ref.wav",
      },
    });
  });

  it("maps MOSS private debug parameters", () => {
    expect(
      buildTtsInvokeConfig({
        voice: " Ava ",
        samplingMode: "fixed",
        referenceAudioPath: "",
        seed: "42",
        maxNewFrames: "128",
        textTemperature: "1.1",
        textTopP: "0.9",
        textTopK: "40",
        audioTemperature: "0.7",
        audioTopP: "0.85",
        audioTopK: "20",
        audioRepetitionPenalty: "1.15",
      }),
    ).toEqual({
      voice: "Ava",
      moss: {
        samplingMode: "fixed",
        seed: 42,
        maxNewFrames: 128,
        textTemperature: 1.1,
        textTopP: 0.9,
        textTopK: 40,
        audioTemperature: 0.7,
        audioTopP: 0.85,
        audioTopK: 20,
        audioRepetitionPenalty: 1.15,
      },
    });
  });

  it("omits invalid numeric fields and floors non-negative integers", () => {
    expect(
      buildTtsInvokeConfig({
        ...defaultTtsInput,
        seed: "-1.2",
        maxNewFrames: "12.9",
        textTemperature: "NaN",
        textTopK: "5.8",
        audioTopP: "Infinity",
        audioTopK: "bad",
      }),
    ).toEqual({
      moss: {
        samplingMode: "fixed",
        seed: 0,
        maxNewFrames: 12,
        textTopK: 5,
      },
    });
  });
});

describe("buildDebugStreamingAsrRequest", () => {
  it("trims source and omits empty optional fields", () => {
    expect(
      buildDebugStreamingAsrRequest("run-1", "file", "  /tmp/audio.wav  ", undefined, "", "", "", ""),
    ).toEqual({
      runId: "run-1",
      sourceKind: "file",
      source: "/tmp/audio.wav",
    });
  });

  it("maps streaming ASR tuning fields", () => {
    expect(
      buildDebugStreamingAsrRequest("run-2", "url", "https://x.test/a.wav", undefined, " en ", "2.5", "2", "5"),
    ).toEqual({
      runId: "run-2",
      sourceKind: "url",
      source: "https://x.test/a.wav",
      language: "en",
      chunkSeconds: 2.5,
      unfixedChunkNum: 2,
      unfixedTokenNum: 5,
    });
  });

  it("floors integer tuning fields", () => {
    expect(
      buildDebugStreamingAsrRequest("run-3", "file", "/tmp/a.wav", undefined, "", "", "2.9", "5.8"),
    ).toMatchObject({
      unfixedChunkNum: 2,
      unfixedTokenNum: 5,
    });
  });

  it("includes selected file bytes", () => {
    expect(
      buildDebugStreamingAsrRequest("run-4", "file", "picked.wav", [1, 2, 3], "", "", "", ""),
    ).toMatchObject({
      runId: "run-4",
      sourceKind: "file",
      source: "picked.wav",
      audioData: [1, 2, 3],
    });
  });
});

describe("formatAsrDebugTime", () => {
  it("formats known seconds", () => {
    expect(formatAsrDebugTime(1.234)).toBe("1.23s");
  });

  it("formats missing seconds", () => {
    expect(formatAsrDebugTime(null)).toBe("--");
    expect(formatAsrDebugTime(undefined)).toBe("--");
  });
});

describe("formatTtsDebugTime", () => {
  it("uses the same compact seconds display as streaming ASR", () => {
    expect(formatTtsDebugTime(2.345)).toBe("2.35s");
    expect(formatTtsDebugTime(undefined)).toBe("--");
  });
});
