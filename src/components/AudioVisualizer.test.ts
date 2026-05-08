import { describe, expect, it } from "vitest";
import { getVadStatusLabel } from "./AudioVisualizer";

describe("getVadStatusLabel", () => {
  it("shows concise listening copy", () => {
    expect(getVadStatusLabel("listening")).toBe("Listening");
  });

  it("keeps processing copy while current utterance is transcribed", () => {
    expect(getVadStatusLabel("processing")).toBe("Processing...");
  });
});
