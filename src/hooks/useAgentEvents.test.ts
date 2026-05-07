import { describe, expect, it } from "vitest";
import {
  appendAgentEvent,
  initialAgentEventsState,
  reduceAgentEvent,
  type AgentEvent,
} from "./useAgentEvents";

function event(overrides: Partial<AgentEvent>): AgentEvent {
  return {
    id: "event-1",
    kind: "result",
    content: "hello",
    createdAt: 1,
    ...overrides,
  };
}

describe("appendAgentEvent", () => {
  it("appends chunks with the same ACP message id to the existing event", () => {
    const current = [
      event({
        id: "event-1",
        messageId: "550e8400-e29b-41d4-a716-446655440000",
        content: "hel",
      }),
    ];

    const next = event({
      id: "event-2",
      messageId: "550e8400-e29b-41d4-a716-446655440000",
      content: "lo",
      createdAt: 2,
    });

    expect(appendAgentEvent(current, next)).toEqual([
      {
        ...current[0],
        content: "hello",
        createdAt: 2,
      },
    ]);
  });

  it("keeps events without ACP message ids as separate output blocks", () => {
    const current = [event({ id: "event-1", content: "first" })];
    const next = event({ id: "event-2", content: "second" });

    expect(appendAgentEvent(current, next)).toEqual([...current, next]);
  });

  it("merges stream text events without message id when operation is append", () => {
    const current = [
      event({
        id: "event-1",
        kind: "thinking",
        operation: "append",
        content: "I am",
      }),
    ];
    const next = event({
      id: "event-2",
      kind: "thinking",
      operation: "append",
      content: " thinking",
      createdAt: 2,
    });

    expect(appendAgentEvent(current, next)).toEqual([
      {
        ...current[0],
        content: "I am thinking",
        createdAt: 2,
      },
    ]);
  });

  it("keeps non-stream append events without message id as separate blocks", () => {
    const current = [
      event({
        id: "event-1",
        kind: "status",
        operation: "append",
        content: "alpha",
      }),
    ];
    const next = event({
      id: "event-2",
      kind: "status",
      operation: "append",
      content: "beta",
      createdAt: 2,
    });

    expect(appendAgentEvent(current, next)).toEqual([...current, next]);
  });

  it("merges overlapping append chunks with the same ACP message id", () => {
    const current = [
      event({
        id: "event-1",
        kind: "result",
        messageId: "550e8400-e29b-41d4-a716-446655440000",
        operation: "append",
        content: "看起来",
        contentBlocks: [{ kind: "text", summary: "看起来", text: "看起来" }],
      }),
    ];

    const next = event({
      id: "event-2",
      kind: "result",
      messageId: "550e8400-e29b-41d4-a716-446655440000",
      operation: "append",
      content: "看起来是语音识别的误识别结果",
      contentBlocks: [
        {
          kind: "text",
          summary: "看起来是语音识别的误识别结果",
          text: "看起来是语音识别的误识别结果",
        },
      ],
      createdAt: 2,
    });

    expect(appendAgentEvent(current, next)).toEqual([
      {
        ...current[0],
        content: "看起来是语音识别的误识别结果",
        contentBlocks: [
          { kind: "text", summary: "看起来", text: "看起来" },
          {
            kind: "text",
            summary: "看起来是语音识别的误识别结果",
            text: "看起来是语音识别的误识别结果",
          },
        ],
        createdAt: 2,
      },
    ]);
  });

  it("keeps different kinds with the same message id as separate blocks", () => {
    const current = [
      event({
        id: "event-1",
        kind: "thinking",
        messageId: "550e8400-e29b-41d4-a716-446655440000",
        content: "thinking",
      }),
    ];
    const next = event({
      id: "event-2",
      kind: "result",
      messageId: "550e8400-e29b-41d4-a716-446655440000",
      content: "answer",
    });

    expect(appendAgentEvent(current, next)).toEqual([...current, next]);
  });

  it("keeps same-message result chunks merged when they form a complete TTS block", () => {
    const current = [
      event({
        id: "event-1",
        kind: "result",
        messageId: "550e8400-e29b-41d4-a716-446655440000",
        operation: "append",
        content: "完成 <tts>我",
      }),
    ];

    const next = event({
      id: "event-2",
      kind: "result",
      messageId: "550e8400-e29b-41d4-a716-446655440000",
      operation: "append",
      content: "处理好了</tts> 正文",
      createdAt: 2,
    });

    expect(appendAgentEvent(current, next)).toEqual([
      {
        ...current[0],
        content: "完成 <tts>我处理好了</tts> 正文",
        createdAt: 2,
      },
    ]);
  });

  it("ignores duplicated events with the same id", () => {
    const current = [
      event({
        id: "event-1",
        kind: "result",
        messageId: "550e8400-e29b-41d4-a716-446655440000",
        operation: "append",
        content: "done",
      }),
    ];
    const duplicate = event({
      id: "event-1",
      kind: "result",
      messageId: "550e8400-e29b-41d4-a716-446655440000",
      operation: "append",
      content: "done",
      createdAt: 2,
    });

    expect(appendAgentEvent(current, duplicate)).toEqual(current);
  });
});

describe("reduceAgentEvent", () => {
  it("updates an existing tool block by ACP tool call id", () => {
    const created = event({
      id: "tool-create",
      kind: "tool",
      toolCallId: "tool-1",
      title: "Run tests",
      content: "status: pending",
      tool: {
        toolCallId: "tool-1",
        title: "Run tests",
        kind: "execute",
        status: "pending",
      },
    });
    const updated = event({
      id: "tool-update",
      kind: "tool",
      toolCallId: "tool-1",
      content: "status: completed",
      createdAt: 2,
      tool: {
        toolCallId: "tool-1",
        status: "completed",
        content: [{ kind: "text", summary: "passed" }],
      },
    });

    const state = reduceAgentEvent(
      reduceAgentEvent(initialAgentEventsState, created),
      updated,
    );

    expect(state.events).toHaveLength(1);
    expect(state.events[0]).toMatchObject({
      id: "tool-create",
      content: "status: completed",
      tool: {
        title: "Run tests",
        kind: "execute",
        status: "completed",
        content: [{ kind: "text", summary: "passed" }],
      },
    });
  });

  it("creates a fallback tool block when update arrives first", () => {
    const state = reduceAgentEvent(
      initialAgentEventsState,
      event({
        id: "tool-update",
        kind: "tool",
        toolCallId: "tool-1",
        content: "status: in_progress",
        tool: {
          toolCallId: "tool-1",
          status: "in_progress",
        },
      }),
    );

    expect(state.events).toHaveLength(1);
    expect(state.events[0].tool?.status).toBe("in_progress");
  });

  it("replaces the current plan snapshot without appending output", () => {
    const first = reduceAgentEvent(initialAgentEventsState, event({
      kind: "status",
      content: "Plan updated",
      plan: {
        entries: [{ content: "One", priority: "high", status: "pending" }],
      },
    }));
    const second = reduceAgentEvent(first, event({
      kind: "status",
      content: "Plan updated",
      plan: {
        entries: [{ content: "Two", priority: "medium", status: "completed" }],
      },
    }));

    expect(second.events).toEqual([]);
    expect(second.plan?.entries).toEqual([
      { content: "Two", priority: "medium", status: "completed" },
    ]);
  });

  it("stores session state updates outside the output log", () => {
    const state = reduceAgentEvent(initialAgentEventsState, event({
      kind: "status",
      content: "Session state",
      sessionState: {
        currentModeId: "build",
        availableCommands: [
          { name: "create_plan", description: "Create a plan" },
        ],
        configOptions: [{ id: "approval" }],
        sessionInfo: { title: "Voice task", updatedAt: "2026-04-28T00:00:00Z" },
      },
    }));

    expect(state.events).toEqual([]);
    expect(state.sessionState.currentModeId).toBe("build");
    expect(state.sessionState.availableCommands[0].name).toBe("create_plan");
    expect(state.sessionState.configOptions).toEqual([{ id: "approval" }]);
    expect(state.sessionState.sessionInfo.title).toBe("Voice task");
  });
});
