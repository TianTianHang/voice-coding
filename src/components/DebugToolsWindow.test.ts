import { describe, expect, it } from "vitest";
import { buildTtsInvokeConfig } from "./DebugToolsWindow";

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
