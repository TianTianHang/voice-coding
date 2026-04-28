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

export type AgentEventOperation =
  | "append"
  | "create"
  | "update"
  | "replace"
  | "sessionState"
  | "fallback";

export type ConfirmStatus = "pending" | "accepted" | "rejected";

export type AgentContentBlock = {
  kind: string;
  summary: string;
  text?: string;
  mimeType?: string;
  uri?: string;
  name?: string;
  raw?: unknown;
};

export type AgentDiff = {
  path: string;
  oldText?: string;
  newText: string;
};

export type AgentTerminalRef = {
  terminalId: string;
};

export type AgentToolContent = {
  kind: string;
  summary: string;
  content?: AgentContentBlock;
  diff?: AgentDiff;
  terminal?: AgentTerminalRef;
};

export type AgentToolLocation = {
  path: string;
  line?: number;
};

export type AgentToolPayload = {
  toolCallId: string;
  title?: string;
  kind?: string;
  status?: string;
  content?: AgentToolContent[];
  locations?: AgentToolLocation[];
  rawInput?: unknown;
  rawOutput?: unknown;
};

export type AgentPlanEntry = {
  content: string;
  priority: string;
  status: string;
};

export type AgentPlanSnapshot = {
  entries: AgentPlanEntry[];
};

export type AgentAvailableCommand = {
  name: string;
  description: string;
  inputHint?: string;
};

export type AgentSessionInfo = {
  title?: string | null;
  updatedAt?: string | null;
};

export type AgentSessionStateUpdate = {
  availableCommands?: AgentAvailableCommand[];
  currentModeId?: string;
  configOptions?: unknown[];
  sessionInfo?: AgentSessionInfo;
};

export type AgentSessionState = {
  availableCommands: AgentAvailableCommand[];
  currentModeId?: string;
  configOptions: unknown[];
  sessionInfo: AgentSessionInfo;
};

export type AgentEvent = {
  id: string;
  kind: AgentEventKind;
  messageId?: string;
  toolCallId?: string;
  operation?: AgentEventOperation;
  title?: string;
  content: string;
  contentBlocks?: AgentContentBlock[];
  tool?: AgentToolPayload;
  diff?: AgentDiff;
  terminal?: AgentTerminalRef;
  plan?: AgentPlanSnapshot;
  sessionState?: AgentSessionStateUpdate;
  raw?: unknown;
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

export type AgentEventsState = {
  events: AgentEvent[];
  plan?: AgentPlanSnapshot;
  sessionState: AgentSessionState;
};

export const initialAgentEventsState: AgentEventsState = {
  events: [],
  sessionState: {
    availableCommands: [],
    configOptions: [],
    sessionInfo: {},
  },
};

export interface AgentEventsResult {
  events: AgentEvent[];
  plan?: AgentPlanSnapshot;
  sessionState: AgentSessionState;
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

export function appendAgentEvent(
  currentEvents: AgentEvent[],
  nextEvent: AgentEvent,
): AgentEvent[] {
  if (currentEvents.some((event) => event.id === nextEvent.id)) {
    return currentEvents;
  }

  if (nextEvent.kind === "tool" && nextEvent.toolCallId) {
    return upsertToolEvent(currentEvents, nextEvent);
  }

  const existingIndex = findAppendableEventIndex(currentEvents, nextEvent);

  if (existingIndex === -1) {
    return [...currentEvents, nextEvent];
  }

  return currentEvents.map((event, index) =>
    index === existingIndex
      ? {
          ...event,
          content: mergeEventContent(event, nextEvent),
          contentBlocks: mergeContentBlocks(event, nextEvent),
          createdAt: nextEvent.createdAt,
        }
      : event,
  );
}

function findAppendableEventIndex(
  currentEvents: AgentEvent[],
  nextEvent: AgentEvent,
): number {
  if (nextEvent.messageId) {
    for (let index = currentEvents.length - 1; index >= 0; index -= 1) {
      const event = currentEvents[index];
      if (
        event.messageId === nextEvent.messageId &&
        event.kind === nextEvent.kind
      ) {
        return index;
      }
    }
  }

  if (isStreamTextLikeEvent(nextEvent)) {
    for (let index = currentEvents.length - 1; index >= 0; index -= 1) {
      const event = currentEvents[index];
      if (canMergeAsStreamText(event, nextEvent)) {
        return index;
      }
      if (isStreamTextLikeEvent(event)) {
        break;
      }
    }
  }

  return -1;
}

function isStreamTextLikeEvent(event: AgentEvent): boolean {
  return (
    (event.kind === "thinking" || event.kind === "result") &&
    !event.toolCallId &&
    !event.diff &&
    !event.terminal &&
    !event.plan &&
    !event.sessionState &&
    !event.confirmationId
  );
}

function canMergeAsStreamText(current: AgentEvent, next: AgentEvent): boolean {
  if (!isStreamTextLikeEvent(current) || !isStreamTextLikeEvent(next)) {
    return false;
  }

  if (current.kind !== next.kind) {
    return false;
  }

  if ((current.title ?? "") !== (next.title ?? "")) {
    return false;
  }

  return current.operation === "append" && next.operation === "append";
}

export function reduceAgentEvent(
  currentState: AgentEventsState,
  nextEvent: AgentEvent,
): AgentEventsState {
  if (nextEvent.plan) {
    return {
      ...currentState,
      plan: nextEvent.plan,
    };
  }

  if (nextEvent.sessionState) {
    return {
      ...currentState,
      sessionState: mergeSessionState(
        currentState.sessionState,
        nextEvent.sessionState,
      ),
    };
  }

  return {
    ...currentState,
    events: appendAgentEvent(currentState.events, nextEvent),
  };
}

function mergeContentBlocks(
  current: AgentEvent,
  next: AgentEvent,
): AgentContentBlock[] | undefined {
  const currentBlocks = current.contentBlocks ?? [];
  const nextBlocks = next.contentBlocks ?? [];
  if (currentBlocks.length === 0 && nextBlocks.length === 0) {
    return undefined;
  }

  if (next.operation === "replace") {
    return nextBlocks.length > 0 ? nextBlocks : undefined;
  }

  return [...currentBlocks, ...nextBlocks];
}

function mergeEventContent(current: AgentEvent, next: AgentEvent): string {
  if (next.operation === "replace") {
    return next.content;
  }
  return mergeAppendText(current.content, next.content);
}

function mergeAppendText(current: string, next: string): string {
  if (!current) {
    return next;
  }

  if (!next) {
    return current;
  }

  if (next.startsWith(current)) {
    return next;
  }

  if (current.endsWith(next)) {
    return current;
  }

  return `${current}${next}`;
}

function upsertToolEvent(
  currentEvents: AgentEvent[],
  nextEvent: AgentEvent,
): AgentEvent[] {
  const existingIndex = currentEvents.findIndex(
    (event) =>
      event.kind === "tool" && event.toolCallId === nextEvent.toolCallId,
  );

  if (existingIndex === -1) {
    return [...currentEvents, nextEvent];
  }

  return currentEvents.map((event, index) =>
    index === existingIndex ? mergeToolEvent(event, nextEvent) : event,
  );
}

function mergeToolEvent(current: AgentEvent, next: AgentEvent): AgentEvent {
  const currentTool = current.tool;
  const nextTool = next.tool;

  return {
    ...current,
    operation: next.operation ?? current.operation,
    toolCallId: next.toolCallId ?? current.toolCallId,
    title: next.title ?? current.title,
    content: next.content || current.content,
    diff: next.diff ?? current.diff,
    terminal: next.terminal ?? current.terminal,
    raw: next.raw ?? current.raw,
    createdAt: next.createdAt,
    tool:
      currentTool || nextTool ? mergeToolPayload(currentTool, nextTool) : undefined,
  };
}

function mergeToolPayload(
  current?: AgentToolPayload,
  next?: AgentToolPayload,
): AgentToolPayload | undefined {
  if (!current) {
    return next;
  }
  if (!next) {
    return current;
  }

  return {
    ...current,
    ...next,
    title: next.title ?? current.title,
    kind: next.kind ?? current.kind,
    status: next.status ?? current.status,
    content:
      next.content && next.content.length > 0 ? next.content : current.content,
    locations:
      next.locations && next.locations.length > 0
        ? next.locations
        : current.locations,
    rawInput: next.rawInput ?? current.rawInput,
    rawOutput: next.rawOutput ?? current.rawOutput,
  };
}

function mergeSessionState(
  current: AgentSessionState,
  update: AgentSessionStateUpdate,
): AgentSessionState {
  return {
    availableCommands:
      update.availableCommands && update.availableCommands.length > 0
        ? update.availableCommands
        : current.availableCommands,
    currentModeId: update.currentModeId ?? current.currentModeId,
    configOptions:
      update.configOptions && update.configOptions.length > 0
        ? update.configOptions
        : current.configOptions,
    sessionInfo: {
      ...current.sessionInfo,
      ...update.sessionInfo,
    },
  };
}

export function useAgentEvents(): AgentEventsResult {
  const [agentEventsState, setAgentEventsState] = useState<AgentEventsState>(
    initialAgentEventsState,
  );
  const [status, setStatus] = useState<AgentStatus>({ connected: false });
  const [connectionState, setConnectionState] =
    useState<AgentConnectionState>("disconnected");

  useEffect(() => {
    let disposed = false;
    let unlistenEvent: (() => void) | null = null;
    let unlistenStatus: (() => void) | null = null;

    async function setup() {
      const current = await invoke<AgentStatus>("get_agent_status");
      if (disposed) {
        return;
      }
      setStatus(current);
      setConnectionState(statusToState(current));

      const nextUnlistenEvent = await listen<AgentEvent>("agent-event", (event) => {
        setAgentEventsState((currentState) =>
          reduceAgentEvent(currentState, event.payload),
        );
        if (event.payload.kind === "error") {
          setConnectionState("error");
        }
      });
      if (disposed) {
        nextUnlistenEvent();
        return;
      }
      unlistenEvent = nextUnlistenEvent;

      const nextUnlistenStatus = await listen<AgentStatus>("agent-status", (event) => {
        setStatus(event.payload);
        setConnectionState(statusToState(event.payload));
      });
      if (disposed) {
        nextUnlistenStatus();
        return;
      }
      unlistenStatus = nextUnlistenStatus;
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
      setAgentEventsState((currentState) => ({
        ...currentState,
        events: currentState.events.map((event) =>
          eventMatchesConfirmation(event, confirmationId)
            ? {
                ...event,
                confirmStatus: accepted ? "accepted" : "rejected",
              }
            : event,
        ),
      }));
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
    events: agentEventsState.events,
    plan: agentEventsState.plan,
    sessionState: agentEventsState.sessionState,
    connectionState,
    connectionLabel,
    connect,
    disconnect,
    respondToConfirmation,
  };
}
