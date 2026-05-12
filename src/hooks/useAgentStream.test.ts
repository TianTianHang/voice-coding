import { describe, expect, it } from "vitest";
import {
  initialAgentStreamState,
  reduceAgentTimelinePatch,
  timelineItemToAgentEventKind,
  type AgentTimelineItem,
} from "./useAgentStream";

function item(overrides: Partial<AgentTimelineItem>): AgentTimelineItem {
  return {
    id: "item-1",
    kind: "message",
    sequence: 1,
    content: "hello",
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe("reduceAgentTimelinePatch", () => {
  it("resets from a backend snapshot", () => {
    const state = reduceAgentTimelinePatch(initialAgentStreamState, {
      type: "reset",
      snapshot: {
        sessionId: "session-1",
        currentTurnId: "turn-1",
        sequence: 3,
        items: [item({ id: "message-1" })],
        sessionState: {
          availableCommands: [],
          configOptions: [],
          sessionInfo: { title: "Task" },
        },
        pendingConfirmations: [],
      },
    });

    expect(state.sessionId).toBe("session-1");
    expect(state.items[0].id).toBe("message-1");
    expect(state.sessionState.sessionInfo.title).toBe("Task");
  });

  it("mechanically upserts timeline items", () => {
    const first = reduceAgentTimelinePatch(initialAgentStreamState, {
      type: "upsertItem",
      item: item({ id: "message-1", content: "hel" }),
    });
    const second = reduceAgentTimelinePatch(first, {
      type: "upsertItem",
      item: item({ id: "message-1", sequence: 2, content: "hello" }),
    });

    expect(second.items).toHaveLength(1);
    expect(second.items[0].content).toBe("hello");
    expect(second.sequence).toBe(2);
  });

  it("keeps a newer patch when an older startup snapshot arrives later", () => {
    const patched = reduceAgentTimelinePatch(initialAgentStreamState, {
      type: "upsertItem",
      item: item({ id: "message-2", sequence: 5, content: "new" }),
    });
    const olderSnapshot = {
      sessionId: "session-1",
      sequence: 4,
      items: [item({ id: "message-1", sequence: 4, content: "old" })],
      sessionState: {
        availableCommands: [],
        configOptions: [],
        sessionInfo: {},
      },
      pendingConfirmations: [],
    };
    const guarded =
      olderSnapshot.sequence >= patched.sequence
        ? reduceAgentTimelinePatch(patched, {
            type: "reset",
            snapshot: olderSnapshot,
          })
        : patched;

    expect(guarded.items[0].id).toBe("message-2");
    expect(guarded.sequence).toBe(5);
  });

  it("replaces plan and merges session state", () => {
    const planned = reduceAgentTimelinePatch(initialAgentStreamState, {
      type: "updatePlan",
      plan: {
        entries: [{ content: "One", priority: "high", status: "pending" }],
      },
    });
    const state = reduceAgentTimelinePatch(planned, {
      type: "updateSessionState",
      sessionState: {
        availableCommands: [{ name: "plan", description: "Plan" }],
        currentModeId: "build",
        configOptions: [{ id: "approval" }],
        sessionInfo: { updatedAt: "2026-05-12T00:00:00Z" },
      },
    });

    expect(state.plan?.entries[0].content).toBe("One");
    expect(state.sessionState.currentModeId).toBe("build");
    expect(state.sessionState.availableCommands[0].name).toBe("plan");
  });

  it("tracks and resolves pending confirmations without optimistic updates", () => {
    const pending = reduceAgentTimelinePatch(initialAgentStreamState, {
      type: "upsertItem",
      item: item({
        id: "confirmation:confirm-1",
        kind: "confirmation",
        confirmation: {
          confirmationId: "confirm-1",
          status: "pending",
          content: "Apply?",
        },
      }),
    });
    const resolved = reduceAgentTimelinePatch(pending, {
      type: "resolveConfirmation",
      confirmationId: "confirm-1",
      status: "accepted",
    });

    expect(pending.pendingConfirmations).toHaveLength(1);
    expect(resolved.pendingConfirmations).toHaveLength(0);
    expect(resolved.items[0].confirmation?.status).toBe("accepted");
  });
});

describe("timelineItemToAgentEventKind", () => {
  it("maps backend UI kinds to legacy display buckets", () => {
    expect(timelineItemToAgentEventKind("message")).toBe("result");
    expect(timelineItemToAgentEventKind("confirmation")).toBe("confirm");
    expect(timelineItemToAgentEventKind("fallback")).toBe("status");
  });
});
