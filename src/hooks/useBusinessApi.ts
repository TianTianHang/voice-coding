import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AppReadiness = "initializing" | "ready" | "degraded" | "failed";
export type VoiceInputMode =
  | "dictationOnly"
  | "autoSendToAgent"
  | "confirmBeforeSend";
export type VoiceSessionState =
  | "idle"
  | "starting"
  | "listening"
  | "recording"
  | "transcribing"
  | "paused"
  | "stopping"
  | "failed";
export type VoicePauseReason = "ttsPlayback" | "user";
export type VoiceUtteranceKind =
  | "detected"
  | "transcribed"
  | "submittedToAgent"
  | "discarded"
  | "failed";
export type AgentMessageSource =
  | "manual"
  | "voice"
  | "editedTranscript"
  | "retry";
export type AgentTurnState = "running" | "completed" | "failed" | "cancelled";
export type SpeechOutputState =
  | "idle"
  | "synthesizing"
  | "ready"
  | "playing"
  | "stopping"
  | "failed";
export type SpeechOutputSource = "text" | "agentResult" | "autoAgentResult";

export type VoiceSessionConfig = {
  inputMode: VoiceInputMode;
};

export type SpeechPreferences = {
  autoSpeakAgentResults: boolean;
};

export type AppPreferences = {
  voice: VoiceSessionConfig;
  speech: SpeechPreferences;
};

export type VoiceSessionStatus = {
  sessionId?: number;
  state: VoiceSessionState;
  pauseReason?: VoicePauseReason;
  error?: string;
  config: VoiceSessionConfig;
};

export type AgentStatus = {
  connected: boolean;
  profileName?: string;
  sessionId?: string;
  error?: string;
};

export type AgentTurnStatus = {
  turnId: string;
  state: AgentTurnState;
  source: AgentMessageSource;
  utteranceId?: string;
  createdAt: number;
  updatedAt: number;
  error?: string;
};

export type SpeechOutputStatus = {
  speechId?: string;
  state: SpeechOutputState;
  source?: SpeechOutputSource;
  autoSpeakAgentResults: boolean;
  error?: string;
};

export type VoiceUtteranceEvent = {
  kind: VoiceUtteranceKind;
  sessionId: number;
  utteranceId: string;
  transcript?: string;
  originalTranscript?: string;
  turnId?: string;
  error?: string;
  createdAt: number;
};

export type RuntimeErrorEvent = {
  scope: string;
  message: string;
  recoverable: boolean;
  createdAt: number;
};

export type AppStatus = {
  readiness: AppReadiness;
  error?: string;
  asr: unknown;
  tts: unknown;
  voice: VoiceSessionStatus;
  agent: AgentStatus;
  speech: SpeechOutputStatus;
  preferences: AppPreferences;
};

export type SendAgentMessageRequest = {
  text: string;
  source: AgentMessageSource;
  utteranceId?: string;
};

export interface BusinessApiResult {
  status: AppStatus | null;
  latestUtterance: VoiceUtteranceEvent | null;
  latestError: RuntimeErrorEvent | null;
  refresh: () => Promise<void>;
  prepare: () => Promise<AppStatus>;
  startVoiceSession: () => Promise<VoiceSessionStatus>;
  stopVoiceSession: () => Promise<VoiceSessionStatus>;
  updateVoiceConfig: (
    config: VoiceSessionConfig,
  ) => Promise<VoiceSessionStatus>;
  sendAgentMessage: (
    request: SendAgentMessageRequest,
  ) => Promise<AgentTurnStatus>;
  speakText: (text: string) => Promise<SpeechOutputStatus>;
  stopSpeech: () => Promise<SpeechOutputStatus>;
}

export function useBusinessApi(): BusinessApiResult {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [latestUtterance, setLatestUtterance] =
    useState<VoiceUtteranceEvent | null>(null);
  const [latestError, setLatestError] = useState<RuntimeErrorEvent | null>(
    null,
  );

  const refresh = useCallback(async () => {
    setStatus(await invoke<AppStatus>("get_app_status"));
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisteners: Array<() => void> = [];

    async function setup() {
      const listeners = await Promise.all([
        listen<AppStatus>("app-status-changed", (event) => {
          setStatus(event.payload);
        }),
        listen<VoiceSessionStatus>("voice-session-changed", (event) => {
          setStatus((current) =>
            current ? { ...current, voice: event.payload } : current,
          );
        }),
        listen<AgentStatus>("agent-status-changed", (event) => {
          setStatus((current) =>
            current ? { ...current, agent: event.payload } : current,
          );
        }),
        listen<SpeechOutputStatus>("speech-output-changed", (event) => {
          setStatus((current) =>
            current ? { ...current, speech: event.payload } : current,
          );
        }),
        listen<VoiceUtteranceEvent>("voice-utterance", (event) => {
          setLatestUtterance(event.payload);
        }),
        listen<RuntimeErrorEvent>("runtime-error", (event) => {
          setLatestError(event.payload);
        }),
      ]);

      if (disposed) {
        listeners.forEach((unlisten) => unlisten());
        return;
      }
      unlisteners = listeners;
      await refresh();
    }

    setup().catch((error) => {
      setLatestError({
        scope: "app",
        message: String(error),
        recoverable: true,
        createdAt: Date.now(),
      });
    });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
      unlisteners = [];
    };
  }, [refresh]);

  const prepare = useCallback(async () => {
    const next = await invoke<AppStatus>("prepare_app");
    setStatus(next);
    return next;
  }, []);

  const startVoiceSession = useCallback(async () => {
    return invoke<VoiceSessionStatus>("start_voice_session");
  }, []);

  const stopVoiceSession = useCallback(async () => {
    return invoke<VoiceSessionStatus>("stop_voice_session");
  }, []);

  const updateVoiceConfig = useCallback(async (config: VoiceSessionConfig) => {
    return invoke<VoiceSessionStatus>("update_voice_session_config", {
      config,
    });
  }, []);

  const sendAgentMessage = useCallback(
    async (request: SendAgentMessageRequest) => {
      return invoke<AgentTurnStatus>("send_agent_message", { request });
    },
    [],
  );

  const speakText = useCallback(async (text: string) => {
    return invoke<SpeechOutputStatus>("speak_text", { request: { text } });
  }, []);

  const stopSpeech = useCallback(async () => {
    return invoke<SpeechOutputStatus>("stop_speech");
  }, []);

  return {
    status,
    latestUtterance,
    latestError,
    refresh,
    prepare,
    startVoiceSession,
    stopVoiceSession,
    updateVoiceConfig,
    sendAgentMessage,
    speakText,
    stopSpeech,
  };
}
