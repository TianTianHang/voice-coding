import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentEventStream } from "./AgentEventStream";
import type { AgentEvent } from "../hooks/useAgentEvents";

function event(overrides: Partial<AgentEvent>): AgentEvent {
  return {
    id: "event-1",
    kind: "status",
    content: "content",
    createdAt: 1,
    ...overrides,
  };
}

describe("AgentEventStream", () => {
  it("renders failed tools with content, location, and status", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "tool",
            toolCallId: "tool-1",
            title: "Run tests",
            tool: {
              toolCallId: "tool-1",
              title: "Run tests",
              kind: "execute",
              status: "failed",
              locations: [{ path: "src/App.tsx", line: 12 }],
              content: [{ kind: "text", summary: "failed", content: { kind: "text", summary: "failed", text: "failed" } }],
            },
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("Run tests");
    expect(markup).toContain("Tool");
    expect(markup).toContain("failed");
    expect(markup).toContain("src/App.tsx:12");
  });

  it("renders diff and terminal content without blank blocks", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "tool",
            toolCallId: "tool-1",
            tool: {
              toolCallId: "tool-1",
              content: [
                {
                  kind: "diff",
                  summary: "Diff: src/main.rs",
                  diff: { path: "src/main.rs", oldText: "old", newText: "new" },
                },
                {
                  kind: "terminal",
                  summary: "Terminal: term-1",
                  terminal: { terminalId: "term-1" },
                },
              ],
            },
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("src/main.rs");
    expect(markup).toContain("old");
    expect(markup).toContain("new");
    expect(markup).toContain("Terminal: term-1");
  });

  it("renders unknown fallback status text", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "status",
            title: "Unknown update",
            content: "{\"sessionUpdate\":\"future\"}",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("Unknown update");
    expect(markup).toContain("future");
  });

  it("renders streamed thinking content", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "thinking",
            operation: "append",
            content: "I am thinking",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("Thinking");
    expect(markup).toContain("I am thinking");
  });

  it("keeps the timeline collapsed until explicitly expanded", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "result",
            title: "Answer",
            content: "Detailed response",
          }),
        ]}
        expanded={false}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("Show timeline");
    expect(markup).toContain("Latest: Result");
    expect(markup).not.toContain("Full agent history");
  });

  it("surfaces confirm events in the collapsed timeline summary", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "confirm",
            content: "Apply these changes?",
            confirmationId: "confirm-1",
            confirmStatus: "pending",
          }),
        ]}
        expanded={false}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("1 event stored");
    expect(markup).toContain("Latest: Confirm");
    expect(markup).toContain("Show timeline");
  });
});
