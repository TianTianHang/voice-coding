import type {
  AgentDiff,
  AgentEvent,
  AgentEventKind,
  AgentToolContent,
} from "../hooks/useAgentEvents";

interface AgentEventStreamProps {
  events: AgentEvent[];
  onConfirm: (confirmationId: string, accepted: boolean) => Promise<void>;
}

const eventLabels: Record<AgentEventKind, string> = {
  thinking: "Thinking",
  tool: "Tool",
  result: "Result",
  diff: "Diff",
  confirm: "Confirm",
  error: "Error",
  status: "Status",
};

export function AgentEventStream({
  events,
  onConfirm,
}: AgentEventStreamProps) {
  if (events.length === 0) {
    return (
      <section className="event-stream event-stream-empty">
        <div className="empty-output">Agent output will appear here.</div>
      </section>
    );
  }

  return (
    <section className="event-stream" aria-label="Agent output stream">
      {events.map((event) => (
        <article
          className={`agent-event agent-event-${event.kind} ${
            event.tool?.status === "failed" ? "agent-event-tool-failed" : ""
          }`}
          key={event.id}
        >
          <header className="agent-event-header">
            <span className="agent-event-type">{eventLabels[event.kind]}</span>
            {event.title && <span className="agent-event-title">{event.title}</span>}
            {event.tool?.status && (
              <span className={`tool-status tool-status-${event.tool.status}`}>
                {event.tool.status.replace("_", " ")}
              </span>
            )}
          </header>

          {event.kind === "tool" && event.tool ? (
            <ToolEvent event={event} />
          ) : (
            <div className="agent-event-content">{event.content}</div>
          )}

          {event.diff && <DiffBlock diff={event.diff} />}
          {event.terminal && (
            <div className="terminal-ref">Terminal: {event.terminal.terminalId}</div>
          )}
          {event.contentBlocks?.map((block, index) =>
            block.kind === "text" ? null : (
              <div className="content-placeholder" key={`${block.kind}-${index}`}>
                {block.summary}
              </div>
            ),
          )}

          {event.kind === "confirm" && event.confirmationId && (
            <div className="confirm-actions">
              <button
                className="confirm-button confirm-button-accept"
                disabled={event.confirmStatus !== "pending"}
                onClick={() => onConfirm(event.confirmationId!, true)}
              >
                Confirm
              </button>
              <button
                className="confirm-button confirm-button-reject"
                disabled={event.confirmStatus !== "pending"}
                onClick={() => onConfirm(event.confirmationId!, false)}
              >
                Reject
              </button>
              {event.confirmStatus && event.confirmStatus !== "pending" && (
                <span className="confirm-status">{event.confirmStatus}</span>
              )}
            </div>
          )}
        </article>
      ))}
    </section>
  );
}

function ToolEvent({ event }: { event: AgentEvent }) {
  const tool = event.tool!;

  return (
    <div className="tool-event-body">
      <div className="tool-meta">
        {tool.kind && <span>{tool.kind}</span>}
        {tool.toolCallId && <span>{tool.toolCallId}</span>}
      </div>

      {tool.locations && tool.locations.length > 0 && (
        <div className="tool-locations">
          {tool.locations.map((location) => (
            <span key={`${location.path}:${location.line ?? ""}`}>
              {location.path}
              {location.line ? `:${location.line}` : ""}
            </span>
          ))}
        </div>
      )}

      {tool.content && tool.content.length > 0 ? (
        <div className="tool-content-list">
          {tool.content.map((content, index) => (
            <ToolContentBlock content={content} key={index} />
          ))}
        </div>
      ) : (
        <div className="agent-event-content">{event.content}</div>
      )}
    </div>
  );
}

function ToolContentBlock({ content }: { content: AgentToolContent }) {
  if (content.diff) {
    return <DiffBlock diff={content.diff} />;
  }
  if (content.terminal) {
    return (
      <div className="terminal-ref">
        Terminal: {content.terminal.terminalId}
      </div>
    );
  }
  if (content.content?.text) {
    return <div className="agent-event-content">{content.content.text}</div>;
  }
  return <div className="content-placeholder">{content.summary}</div>;
}

function DiffBlock({ diff }: { diff: AgentDiff }) {
  return (
    <div className="diff-block">
      <div className="diff-path">{diff.path}</div>
      {diff.oldText !== undefined && (
        <pre className="diff-text diff-old">{diff.oldText}</pre>
      )}
      <pre className="diff-text diff-new">{diff.newText}</pre>
    </div>
  );
}
