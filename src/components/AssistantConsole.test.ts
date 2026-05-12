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
        voice: { state: "recording", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: true,
        agent: { connected: true },
        speech: { state: "idle", autoSpeakAgentResults: true },
      }),
    ).toBe("WakeDetected");
  });

  it("maps active business voice sessions to Listening after wake confirmation", () => {
    expect(
      deriveVoiceExperienceState({
        voice: { state: "recording", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: false,
        agent: { connected: true },
        speech: { state: "idle", autoSpeakAgentResults: true },
      }),
    ).toBe("Listening");
  });

  it("maps processing and agent work to Working before response", () => {
    expect(
      deriveVoiceExperienceState({
        voice: { state: "transcribing", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: false,
        agent: { connected: true },
        speech: { state: "idle", autoSpeakAgentResults: true },
      }),
    ).toBe("Processing");

    expect(
      deriveVoiceExperienceState({
        voice: { state: "listening", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: false,
        agent: { connected: true },
        agentTurn: {
          turnId: "turn-1",
          state: "running",
          source: "voice",
          createdAt: 1,
          updatedAt: 1,
        },
        speech: { state: "idle", autoSpeakAgentResults: true },
        latestAgentEvent: event({ kind: "tool", content: "Running tests" }),
      }),
    ).toBe("Processing");
  });

  it("maps result events to Responding", () => {
    expect(
      deriveVoiceExperienceState({
        voice: { state: "listening", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: false,
        agent: { connected: true },
        speech: { state: "idle", autoSpeakAgentResults: true },
        latestAgentEvent: event({ kind: "result", content: "Done" }),
      }),
    ).toBe("Responding");
  });

  it("prioritizes recoverable errors over all other states", () => {
    expect(
      deriveVoiceExperienceState({
        voice: { state: "recording", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: true,
        agent: { connected: true },
        speech: { state: "failed", error: "Microphone denied", autoSpeakAgentResults: true },
      }),
    ).toBe("Error");

    expect(
      deriveVoiceExperienceState({
        voice: { state: "idle", config: { inputMode: "autoSendToAgent" } },
        wakeDetected: false,
        agent: { connected: false, error: "Agent unavailable" },
        speech: { state: "idle", autoSpeakAgentResults: true },
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
