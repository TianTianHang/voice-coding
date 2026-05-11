import { describe, expect, it } from "vitest";
import {
  buildDebugStreamingAsrRequest,
  buildTtsInvokeConfig,
  formatAsrDebugTime,
} from "./DebugToolsWindow";

describe("buildTtsInvokeConfig", () => {
  it("maps the default debug selection to fixed MOSS sampling", () => {
    expect(buildTtsInvokeConfig("fixed", "")).toEqual({
      moss: { samplingMode: "fixed" },
    });
  });

  it("maps greedy sampling and trims the reference audio path", () => {
    expect(buildTtsInvokeConfig("greedy", "  /tmp/ref.wav  ")).toEqual({
      moss: {
        samplingMode: "greedy",
        referenceAudioPath: "/tmp/ref.wav",
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
