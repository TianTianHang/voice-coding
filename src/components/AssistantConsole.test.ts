import { describe, expect, it } from "vitest";
import {
  deriveVoiceExperienceState,
  shouldExpandTimelineForEvent,
} from "./AssistantConsole";
import type { AgentEvent } from "../hooks/useAgentEvents";

function event(overrides: Partial<AgentEvent>): AgentEvent {
  return {
    id: "event-1",
    kind: "result",
    content: "hello",
    createdAt: 1,
    ...overrides,
  };
}

describe("deriveVoiceExperienceState", () => {
  it("uses WakeDetected as a short activation confirmation", () => {
    expect(
      deriveVoiceExperienceState({
        vadState: "recording",
        wakeDetected: true,
        agentConnectionState: "connected",
      }),
    ).toBe("WakeDetected");
  });

  it("maps active VAD recording to Listening after the wake confirmation", () => {
    expect(
      deriveVoiceExperienceState({
        vadState: "recording",
        wakeDetected: false,
        agentConnectionState: "connected",
      }),
    ).toBe("Listening");
  });

  it("maps processing and agent work to Working before response", () => {
    expect(
      deriveVoiceExperienceState({
        vadState: "processing",
        wakeDetected: false,
        agentConnectionState: "connected",
      }),
    ).toBe("Processing");

    expect(
      deriveVoiceExperienceState({
        vadState: "listening",
        wakeDetected: false,
        agentConnectionState: "connected",
        latestAgentEvent: event({ kind: "tool", content: "Running tests" }),
      }),
    ).toBe("Processing");
  });

  it("maps result events to Responding", () => {
    expect(
      deriveVoiceExperienceState({
        vadState: "listening",
        wakeDetected: false,
        agentConnectionState: "connected",
        latestAgentEvent: event({ kind: "result", content: "Done" }),
      }),
    ).toBe("Responding");
  });

  it("prioritizes recoverable errors over all other states", () => {
    expect(
      deriveVoiceExperienceState({
        vadState: "recording",
        wakeDetected: true,
        speechError: "Microphone denied",
        agentConnectionState: "connected",
      }),
    ).toBe("Error");

    expect(
      deriveVoiceExperienceState({
        vadState: "idle",
        wakeDetected: false,
        agentConnectionState: "error",
      }),
    ).toBe("Error");
  });
});

describe("shouldExpandTimelineForEvent", () => {
  it("auto-expands the timeline for confirm and error events", () => {
    expect(
      shouldExpandTimelineForEvent(
        event({
          kind: "confirm",
          confirmationId: "confirm-1",
          content: "Apply changes?",
        }),
      ),
    ).toBe(true);

    expect(
      shouldExpandTimelineForEvent(
        event({ kind: "error", content: "Agent failed" }),
      ),
    ).toBe(true);
  });

  it("keeps ordinary events in the collapsed timeline path", () => {
    expect(
      shouldExpandTimelineForEvent(
        event({ kind: "result", content: "Done" }),
      ),
    ).toBe(false);
  });
});
