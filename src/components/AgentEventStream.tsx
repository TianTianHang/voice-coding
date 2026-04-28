import type { AgentEvent, AgentEventKind } from "../hooks/useAgentEvents";

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
          className={`agent-event agent-event-${event.kind}`}
          key={event.id}
        >
          <header className="agent-event-header">
            <span className="agent-event-type">{eventLabels[event.kind]}</span>
            {event.title && <span className="agent-event-title">{event.title}</span>}
          </header>
          <div className="agent-event-content">{event.content}</div>
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
