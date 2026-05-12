import { describe, expect, it } from "vitest";
import {
  mergeBusinessStatusEvent,
  type AppStatus,
  type SpeechOutputStatus,
} from "./useBusinessApi";

function appStatus(): AppStatus {
  return {
    readiness: "ready",
    asr: {},
    tts: {},
    voice: {
      state: "idle",
      config: { inputMode: "autoSendToAgent" },
    },
    agent: {
      connected: false,
    },
    speech: {
      state: "idle",
      autoSpeakAgentResults: true,
    },
    preferences: {
      voice: { inputMode: "autoSendToAgent" },
      speech: { autoSpeakAgentResults: true },
    },
  };
}

describe("mergeBusinessStatusEvent", () => {
  it("merges business event payloads into an existing app snapshot", () => {
    const speech: SpeechOutputStatus = {
      state: "playing",
      source: "agentResult",
      speechId: "speech-1",
      autoSpeakAgentResults: true,
    };

    expect(mergeBusinessStatusEvent(appStatus(), "speech", speech)).toMatchObject({
      readiness: "ready",
      speech,
      voice: { state: "idle" },
    });
  });

  it("keeps null snapshots null until get_app_status resolves", () => {
    expect(
      mergeBusinessStatusEvent(null, "agent", {
        connected: true,
        profileName: "default",
      }),
    ).toBeNull();
  });
});
