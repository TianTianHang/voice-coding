import { describe, expect, it } from "vitest";
import {
  autoTtsStatusLabel,
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

describe("autoTtsStatusLabel", () => {
  const base = {
    enabled: true,
    isPlaying: false,
    latestResultText: "Done",
    latestResultKey: "result-1:Done",
    latestSpokenResultKey: "result-1:Done",
    lastStatus: "idle" as const,
    tts: {
      state: "idle" as const,
      engineName: "mock-tts",
      model: {
        kind: "tts" as const,
        modelId: "moss-tts-nano-100m-onnx",
        engineName: "moss-onnx-tts",
        packageDir: "",
        modelDir: "",
        source: "devFallback" as const,
        legacyLayout: false,
        missingFiles: [],
      },
      hasBufferedAudio: true,
    },
  };

  it("distinguishes disabled, idle, speaking, duplicate, and failed states", () => {
    expect(autoTtsStatusLabel(null)).toBe("");
    expect(autoTtsStatusLabel({ ...base, enabled: false, lastStatus: "disabled" })).toBe("自动播报关闭");
    expect(autoTtsStatusLabel(base)).toBe("自动播报已开启");
    expect(autoTtsStatusLabel({ ...base, isPlaying: true, lastStatus: "speaking" })).toBe("正在播报回复");
    expect(autoTtsStatusLabel({ ...base, lastStatus: "skippedDuplicate" })).toBe("重复回复已跳过");
    expect(
      autoTtsStatusLabel({
        ...base,
        lastStatus: "failed",
        tts: { ...base.tts, state: "failed", error: "boom" },
      }),
    ).toBe("自动播报失败：boom");
  });
});
