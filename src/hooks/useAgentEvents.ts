import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AgentEventKind =
  | "thinking"
  | "tool"
  | "result"
  | "diff"
  | "confirm"
  | "error"
  | "status";

export type ConfirmStatus = "pending" | "accepted" | "rejected";

export type AgentEvent = {
  id: string;
  kind: AgentEventKind;
  title?: string;
  content: string;
  confirmationId?: string;
  confirmStatus?: ConfirmStatus;
  createdAt: number;
};

export type AgentConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "error";

type AgentStatus = {
  connected: boolean;
  profileName?: string;
  sessionId?: string;
  error?: string;
};

export interface AgentEventsResult {
  events: AgentEvent[];
  connectionState: AgentConnectionState;
  connectionLabel: string;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  respondToConfirmation: (
    confirmationId: string,
    accepted: boolean,
  ) => Promise<void>;
}

function statusToState(status: AgentStatus): AgentConnectionState {
  if (status.connected) {
    return "connected";
  }
  return status.error ? "error" : "disconnected";
}

function eventMatchesConfirmation(
  event: AgentEvent,
  confirmationId: string,
): boolean {
  return event.kind === "confirm" && event.confirmationId === confirmationId;
}

export function useAgentEvents(): AgentEventsResult {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [status, setStatus] = useState<AgentStatus>({ connected: false });
  const [connectionState, setConnectionState] =
    useState<AgentConnectionState>("disconnected");

  useEffect(() => {
    let disposed = false;
    let unlistenEvent: (() => void) | null = null;
    let unlistenStatus: (() => void) | null = null;

    async function setup() {
      const current = await invoke<AgentStatus>("get_agent_status");
      if (!disposed) {
        setStatus(current);
        setConnectionState(statusToState(current));
      }

      unlistenEvent = await listen<AgentEvent>("agent-event", (event) => {
        setEvents((currentEvents) => [...currentEvents, event.payload]);
        if (event.payload.kind === "error") {
          setConnectionState("error");
        }
      });

      unlistenStatus = await listen<AgentStatus>("agent-status", (event) => {
        setStatus(event.payload);
        setConnectionState(statusToState(event.payload));
      });
    }

    setup().catch((e) => {
      if (!disposed) {
        setStatus({ connected: false, error: String(e) });
        setConnectionState("error");
      }
    });

    return () => {
      disposed = true;
      unlistenEvent?.();
      unlistenStatus?.();
    };
  }, []);

  const connect = useCallback(async () => {
    setConnectionState("connecting");
    try {
      const nextStatus = await invoke<AgentStatus>("connect_agent");
      setStatus(nextStatus);
      setConnectionState(statusToState(nextStatus));
    } catch (e) {
      setStatus({ connected: false, error: String(e) });
      setConnectionState("error");
    }
  }, []);

  const disconnect = useCallback(async () => {
    try {
      const nextStatus = await invoke<AgentStatus>("disconnect_agent");
      setStatus(nextStatus);
      setConnectionState(statusToState(nextStatus));
    } catch (e) {
      setStatus((current) => ({ ...current, error: String(e) }));
      setConnectionState("error");
    }
  }, []);

  const respondToConfirmation = useCallback(
    async (confirmationId: string, accepted: boolean) => {
      await invoke("respond_agent_confirmation", {
        confirmationId,
        accepted,
      });
      setEvents((currentEvents) =>
        currentEvents.map((event) =>
          eventMatchesConfirmation(event, confirmationId)
            ? {
                ...event,
                confirmStatus: accepted ? "accepted" : "rejected",
              }
            : event,
        ),
      );
    },
    [],
  );

  const connectionLabel = useMemo(() => {
    switch (connectionState) {
      case "connecting":
        return "Connecting";
      case "connected":
        return status.profileName
          ? `Connected to ${status.profileName}`
          : "Agent connected";
      case "error":
        return status.error ? `Agent error: ${status.error}` : "Agent error";
      case "disconnected":
        return "Agent disconnected";
    }
  }, [connectionState, status.error, status.profileName]);

  return {
    events,
    connectionState,
    connectionLabel,
    connect,
    disconnect,
    respondToConfirmation,
  };
}
