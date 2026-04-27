import { describe, expect, it } from "vitest";
import {
  appendTranscriptLine,
  resolveSessionForVadStateEvent,
} from "./useBackendVAD";

describe("resolveSessionForVadStateEvent", () => {
  it("adopts a new session when awaiting start", () => {
    const result = resolveSessionForVadStateEvent({
      currentSessionId: null,
      awaitingNewSession: true,
      incomingSessionId: 9,
      incomingState: "listening",
    });

    expect(result).toEqual({
      acceptEvent: true,
      nextSessionId: 9,
      nextAwaitingNewSession: false,
      shouldResetSessionData: true,
    });
  });

  it("keeps active session during processing to listening cycle", () => {
    const result = resolveSessionForVadStateEvent({
      currentSessionId: 9,
      awaitingNewSession: false,
      incomingSessionId: 9,
      incomingState: "listening",
    });

    expect(result).toEqual({
      acceptEvent: true,
      nextSessionId: 9,
      nextAwaitingNewSession: false,
      shouldResetSessionData: false,
    });
  });

  it("drops stale events from a previous session", () => {
    const result = resolveSessionForVadStateEvent({
      currentSessionId: 9,
      awaitingNewSession: false,
      incomingSessionId: 8,
      incomingState: "listening",
    });

    expect(result.acceptEvent).toBe(false);
    expect(result.nextSessionId).toBe(9);
  });

  it("clears active session only on matching idle event", () => {
    const result = resolveSessionForVadStateEvent({
      currentSessionId: 9,
      awaitingNewSession: false,
      incomingSessionId: 9,
      incomingState: "idle",
    });

    expect(result).toEqual({
      acceptEvent: true,
      nextSessionId: null,
      nextAwaitingNewSession: false,
      shouldResetSessionData: false,
    });
  });
});

describe("appendTranscriptLine", () => {
  it("appends multiple utterances in one session", () => {
    const first = appendTranscriptLine("", "hello world");
    const second = appendTranscriptLine(first, "next command");

    expect(second).toBe("hello world\nnext command");
  });
});
