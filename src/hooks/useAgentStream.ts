import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AgentAvailableCommand,
  AgentContentBlock,
  AgentDiff,
  AgentPlanSnapshot,
  AgentTerminalRef,
  AgentToolPayload,
} from "./useAgentEvents";

export type AgentTimelineItemKind =
  | "thinking"
  | "message"
  | "tool"
  | "diff"
  | "confirmation"
  | "error"
  | "status"
  | "fallback";

export type AgentConfirmationStatus = "pending" | "accepted" | "rejected";

export type AgentConfirmationSnapshot = {
  confirmationId: string;
  status: AgentConfirmationStatus;
  turnId?: string;
  content: string;
};

export type AgentSessionInfoState = {
  title?: string;
  updatedAt?: string;
};

export type AgentStreamSessionState = {
  availableCommands: AgentAvailableCommand[];
  currentModeId?: string;
  configOptions: unknown[];
  sessionInfo: AgentSessionInfoState;
};

export type AgentTimelineItem = {
  id: string;
  kind: AgentTimelineItemKind;
  sessionId?: string;
  turnId?: string;
  sequence: number;
  messageId?: string;
  toolCallId?: string;
  title?: string;
  content: string;
  contentBlocks?: AgentContentBlock[];
  tool?: AgentToolPayload;
  diff?: AgentDiff;
  terminal?: AgentTerminalRef;
  raw?: unknown;
  confirmation?: AgentConfirmationSnapshot;
  createdAt: number;
  updatedAt: number;
};

export type AgentTimelineSnapshot = {
  sessionId?: string;
  currentTurnId?: string;
  sequence: number;
  items: AgentTimelineItem[];
  plan?: AgentPlanSnapshot;
  sessionState: AgentStreamSessionState;
  pendingConfirmations: AgentConfirmationSnapshot[];
};

export type AgentTimelinePatch =
  | { type: "reset"; snapshot: AgentTimelineSnapshot }
  | { type: "upsertItem"; item: AgentTimelineItem }
  | { type: "updatePlan"; plan?: AgentPlanSnapshot }
  | { type: "updateSessionState"; sessionState: AgentStreamSessionState }
  | {
      type: "resolveConfirmation";
      confirmationId: string;
      status: AgentConfirmationStatus;
      item?: AgentTimelineItem;
    }
  | { type: "streamError"; item: AgentTimelineItem };

export type AgentStreamState = AgentTimelineSnapshot;

export type AgentStreamResult = AgentStreamState & {
  respondToConfirmation: (
    confirmationId: string,
    accepted: boolean,
  ) => Promise<void>;
};

export const initialAgentStreamState: AgentStreamState = {
  sequence: 0,
  items: [],
  sessionState: {
    availableCommands: [],
    configOptions: [],
    sessionInfo: {},
  },
  pendingConfirmations: [],
};

export function reduceAgentTimelinePatch(
  current: AgentStreamState,
  patch: AgentTimelinePatch,
): AgentStreamState {
  switch (patch.type) {
    case "reset":
      return normalizeSnapshot(patch.snapshot);
    case "upsertItem":
    case "streamError":
      return {
        ...current,
        sequence: Math.max(current.sequence, patch.item.sequence),
        items: upsertTimelineItem(current.items, patch.item),
        pendingConfirmations: mergePendingConfirmation(
          current.pendingConfirmations,
          patch.item.confirmation,
        ),
      };
    case "updatePlan":
      return {
        ...current,
        plan: patch.plan,
      };
    case "updateSessionState":
      return {
        ...current,
        sessionState: mergeSessionState(current.sessionState, patch.sessionState),
      };
    case "resolveConfirmation":
      return {
        ...current,
        items: patch.item
          ? upsertTimelineItem(current.items, patch.item)
          : current.items.map((item) =>
              item.confirmation?.confirmationId === patch.confirmationId
                ? {
                    ...item,
                    confirmation: {
                      ...item.confirmation,
                      status: patch.status,
                    },
                  }
                : item,
            ),
        pendingConfirmations: current.pendingConfirmations.filter(
          (confirmation) => confirmation.confirmationId !== patch.confirmationId,
        ),
      };
  }
}

export function timelineItemToAgentEventKind(
  kind: AgentTimelineItemKind,
): "thinking" | "tool" | "result" | "diff" | "confirm" | "error" | "status" {
  switch (kind) {
    case "message":
      return "result";
    case "confirmation":
      return "confirm";
    case "fallback":
      return "status";
    default:
      return kind;
  }
}

export function useAgentStream(): AgentStreamResult {
  const [state, setState] = useState<AgentStreamState>(initialAgentStreamState);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    async function setup() {
      const nextUnlisten = await listen<AgentTimelinePatch>(
        "agent-timeline-changed",
        (event) => {
          setState((current) =>
            reduceAgentTimelinePatch(current, event.payload),
          );
        },
      );
      if (disposed) {
        nextUnlisten();
        return;
      }
      unlisten = nextUnlisten;

      const snapshot = await invoke<AgentTimelineSnapshot>("get_agent_timeline");
      if (!disposed) {
        setState((current) =>
          snapshot.sequence >= current.sequence ? normalizeSnapshot(snapshot) : current,
        );
      }
    }

    setup().catch((error) => {
      if (!disposed) {
        setState((current) =>
          reduceAgentTimelinePatch(current, {
            type: "streamError",
            item: {
              id: "agent-stream-init-error",
              kind: "error",
              sequence: current.sequence + 1,
              content: String(error),
              createdAt: Date.now(),
              updatedAt: Date.now(),
            },
          }),
        );
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const respondToConfirmation = useCallback(
    async (confirmationId: string, accepted: boolean) => {
      await invoke("respond_agent_stream_confirmation", {
        confirmationId,
        accepted,
      });
    },
    [],
  );

  return {
    ...state,
    respondToConfirmation,
  };
}

function normalizeSnapshot(snapshot: AgentTimelineSnapshot): AgentStreamState {
  return {
    ...initialAgentStreamState,
    ...snapshot,
    items: snapshot.items ?? [],
    sessionState: {
      ...initialAgentStreamState.sessionState,
      ...snapshot.sessionState,
      sessionInfo: {
        ...initialAgentStreamState.sessionState.sessionInfo,
        ...snapshot.sessionState?.sessionInfo,
      },
    },
    pendingConfirmations: snapshot.pendingConfirmations ?? [],
  };
}

function upsertTimelineItem(
  current: AgentTimelineItem[],
  next: AgentTimelineItem,
): AgentTimelineItem[] {
  const existingIndex = current.findIndex((item) => item.id === next.id);
  if (existingIndex === -1) {
    return [...current, next];
  }
  return current.map((item, index) => (index === existingIndex ? next : item));
}

function mergeSessionState(
  current: AgentStreamSessionState,
  next: AgentStreamSessionState,
): AgentStreamSessionState {
  return {
    availableCommands:
      next.availableCommands && next.availableCommands.length > 0
        ? next.availableCommands
        : current.availableCommands,
    currentModeId: next.currentModeId ?? current.currentModeId,
    configOptions:
      next.configOptions && next.configOptions.length > 0
        ? next.configOptions
        : current.configOptions,
    sessionInfo: {
      ...current.sessionInfo,
      ...next.sessionInfo,
    },
  };
}

function mergePendingConfirmation(
  current: AgentConfirmationSnapshot[],
  next?: AgentConfirmationSnapshot,
): AgentConfirmationSnapshot[] {
  if (!next || next.status !== "pending") {
    return current;
  }
  const existingIndex = current.findIndex(
    (confirmation) => confirmation.confirmationId === next.confirmationId,
  );
  if (existingIndex === -1) {
    return [...current, next];
  }
  return current.map((confirmation, index) =>
    index === existingIndex ? next : confirmation,
  );
}
