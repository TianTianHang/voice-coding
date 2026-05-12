import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentEventStream, stripTtsControlBlocks } from "./AgentEventStream";
import type { AgentTimelineItem } from "../hooks/useAgentStream";

function event(overrides: Partial<AgentTimelineItem>): AgentTimelineItem {
  return {
    id: "event-1",
    kind: "status",
    sequence: 1,
    content: "content",
    createdAt: 1,
    updatedAt: 1,
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
    expect(markup).toContain("工具");
    expect(markup).toContain("failed");
    expect(markup).toContain("src/App.tsx:12");
  });

  it("hides a single complete TTS control block in result content", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "message",
            content: "完成了。\n<tts>我处理好了。</tts>\n改动见上方。",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("完成了。");
    expect(markup).toContain("改动见上方。");
    expect(markup).not.toContain("我处理好了。");
    expect(markup).not.toContain("&lt;tts&gt;");
  });

  it("hides multiple complete TTS control blocks in result content", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "message",
            content: "A <tts>one</tts> B <tts>two</tts> C",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("A  B  C");
    expect(markup).not.toContain("one");
    expect(markup).not.toContain("two");
  });

  it("keeps readable text visible when a TTS tag is incomplete", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "message",
            content: "正文 <tts>未完成",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("正文 &lt;tts&gt;未完成");
  });

  it("strips TTS blocks after stream chunks have merged", () => {
    expect(stripTtsControlBlocks("前缀 <tts>准备好了</tts> 后缀")).toBe(
      "前缀  后缀",
    );
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
            content: "I am thinking",
          }),
        ]}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("思考中");
    expect(markup).toContain("I am thinking");
  });

  it("keeps the timeline collapsed until explicitly expanded", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "message",
            title: "Answer",
            content: "Detailed response",
          }),
        ]}
        expanded={false}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("查看轨迹");
    expect(markup).toContain("最新：结果");
    expect(markup).not.toContain("最近 Agent 事件");
  });

  it("surfaces confirm events in the collapsed timeline summary", () => {
    const markup = renderToStaticMarkup(
      <AgentEventStream
        events={[
          event({
            kind: "confirmation",
            content: "Apply these changes?",
            confirmation: {
              confirmationId: "confirm-1",
              status: "pending",
              content: "Apply these changes?",
            },
          }),
        ]}
        expanded={false}
        onConfirm={async () => {}}
      />,
    );

    expect(markup).toContain("已记录 1 条事件");
    expect(markup).toContain("最新：等待确认");
    expect(markup).toContain("查看轨迹");
  });
});
