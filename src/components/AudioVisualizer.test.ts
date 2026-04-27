import { describe, expect, it } from "vitest";
import { getVadStatusLabel } from "./AudioVisualizer";

describe("getVadStatusLabel", () => {
  it("shows active waiting copy for listening", () => {
    expect(getVadStatusLabel("listening")).toBe(
      "Listening (waiting for speech)..."
    );
  });

  it("keeps processing copy while current utterance is transcribed", () => {
    expect(getVadStatusLabel("processing")).toBe("Processing...");
  });
});
