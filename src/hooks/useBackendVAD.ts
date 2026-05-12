import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Compatibility hook for the legacy VAD command/event surface. New frontend
// work should prefer useBusinessApi and the business status events.
export type VADState = "idle" | "listening" | "recording" | "processing";

export interface BackendVADResult {
  state: VADState;
  transcript: string;
  error: string | null;
  activeSessionId: number | null;
  recordingDuration: number;
  startListening: () => Promise<void>;
  stopListening: () => Promise<void>;
}

type VadStateEventPayload = {
  state: VADState;
  sessionId?: number;
};

type TranscriptEventPayload = {
  text: string;
  sessionId?: number;
};

type ErrorEventPayload = {
  message: string;
  sessionId?: number;
};

type SessionResolutionInput = {
  currentSessionId: number | null;
  awaitingNewSession: boolean;
  incomingSessionId?: number;
  incomingState: VADState;
};

type SessionResolution = {
  acceptEvent: boolean;
  nextSessionId: number | null;
  nextAwaitingNewSession: boolean;
  shouldResetSessionData: boolean;
};

export function resolveSessionForVadStateEvent({
  currentSessionId,
  awaitingNewSession,
  incomingSessionId,
  incomingState,
}: SessionResolutionInput): SessionResolution {
  if (typeof incomingSessionId !== "number") {
    return {
      acceptEvent: true,
      nextSessionId: currentSessionId,
      nextAwaitingNewSession: awaitingNewSession,
      shouldResetSessionData: false,
    };
  }

  if (incomingState === "idle") {
    if (currentSessionId !== null && currentSessionId !== incomingSessionId) {
      return {
        acceptEvent: false,
        nextSessionId: currentSessionId,
        nextAwaitingNewSession: awaitingNewSession,
        shouldResetSessionData: false,
      };
    }

    return {
      acceptEvent: true,
      nextSessionId: null,
      nextAwaitingNewSession: false,
      shouldResetSessionData: false,
    };
  }

  if (currentSessionId === incomingSessionId) {
    return {
      acceptEvent: true,
      nextSessionId: currentSessionId,
      nextAwaitingNewSession: awaitingNewSession,
      shouldResetSessionData: false,
    };
  }

  if (awaitingNewSession) {
    return {
      acceptEvent: true,
      nextSessionId: incomingSessionId,
      nextAwaitingNewSession: false,
      shouldResetSessionData: true,
    };
  }

  return {
    acceptEvent: false,
    nextSessionId: currentSessionId,
    nextAwaitingNewSession: awaitingNewSession,
    shouldResetSessionData: false,
  };
}

export function appendTranscriptLine(previous: string, nextText: string): string {
  return previous ? `${previous}\n${nextText}` : nextText;
}

export function replaceCurrentUtterance(_previous: string, nextText: string): string {
  return nextText;
}

export function useBackendVAD(): BackendVADResult {
  const [state, setState] = useState<VADState>("idle");
  const [transcript, setTranscript] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [recordingDuration, setRecordingDuration] = useState(0);
  const durationRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const activeSessionIdRef = useRef<number | null>(null);
  const isMountedRef = useRef(true);
  const awaitingNewSessionRef = useRef(false);

  const stopDurationTimer = useCallback(() => {
    if (durationRef.current) {
      clearInterval(durationRef.current);
      durationRef.current = null;
    }
    setRecordingDuration(0);
  }, []);

  const setSession = useCallback((sessionId: number | null) => {
    activeSessionIdRef.current = sessionId;
    setActiveSessionId(sessionId);
  }, []);

  useEffect(() => {
    isMountedRef.current = true;
    let disposed = false;
    let unlisteners: Array<() => void> = [];

    async function setup() {
      const listeners = await Promise.all([
        listen<VadStateEventPayload>("vad-state", (event) => {
          if (!isMountedRef.current) {
            return;
          }

          const { state: newState, sessionId } = event.payload;

          const sessionResolution = resolveSessionForVadStateEvent({
            currentSessionId: activeSessionIdRef.current,
            awaitingNewSession: awaitingNewSessionRef.current,
            incomingSessionId: sessionId,
            incomingState: newState,
          });

          if (!sessionResolution.acceptEvent) {
            return;
          }

          if (sessionResolution.nextSessionId !== activeSessionIdRef.current) {
            setSession(sessionResolution.nextSessionId);
          }

          if (sessionResolution.shouldResetSessionData) {
            setTranscript("");
            setError(null);
          }

          awaitingNewSessionRef.current =
            sessionResolution.nextAwaitingNewSession;

          setState(newState);

          if (newState === "recording") {
            const start = Date.now();
            stopDurationTimer();
            durationRef.current = setInterval(() => {
              if (!isMountedRef.current) {
                return;
              }
              setRecordingDuration((Date.now() - start) / 1000);
            }, 100);
          } else {
            stopDurationTimer();
          }

          if (newState === "listening") {
            setTranscript("");
          }
        }),
        listen<TranscriptEventPayload>("transcript", (event) => {
          if (!isMountedRef.current) {
            return;
          }
          const { text, sessionId } = event.payload;
          if (
            typeof sessionId !== "number" ||
            activeSessionIdRef.current !== sessionId
          ) {
            return;
          }
          setTranscript((prev) => replaceCurrentUtterance(prev, text));
        }),
        listen<ErrorEventPayload | string>("error", (event) => {
          if (!isMountedRef.current) {
            return;
          }

          if (typeof event.payload === "string") {
            if (activeSessionIdRef.current !== null) {
              setError(event.payload);
            }
            return;
          }

          const { message, sessionId } = event.payload;
          if (
            typeof sessionId !== "number" ||
            activeSessionIdRef.current !== sessionId
          ) {
            return;
          }

          setError(message);
        }),
      ]);

      if (disposed) {
        listeners.forEach((unlisten) => unlisten());
        return;
      }

      unlisteners = listeners;
    }

    setup().catch((e) => {
      if (isMountedRef.current) {
        setError(String(e));
      }
    });

    return () => {
      disposed = true;
      isMountedRef.current = false;
      unlisteners.forEach((unlisten) => unlisten());
      unlisteners = [];
      stopDurationTimer();
      activeSessionIdRef.current = null;
    };
  }, [setSession, stopDurationTimer]);

  const startListening = useCallback(async () => {
    setError(null);
    awaitingNewSessionRef.current = true;
    try {
      await invoke("start_listening");
    } catch (e) {
      setError(String(e));
      awaitingNewSessionRef.current = false;
    }
  }, []);

  const stopListening = useCallback(async () => {
    try {
      await invoke("stop_listening");
    } catch (e) {
      setError(String(e));
    }
    awaitingNewSessionRef.current = false;
  }, []);

  return {
    state,
    transcript,
    error,
    activeSessionId,
    recordingDuration,
    startListening,
    stopListening,
  };
}
