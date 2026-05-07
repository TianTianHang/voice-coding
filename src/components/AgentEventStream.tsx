import type {
  AgentDiff,
  AgentEvent,
  AgentEventKind,
  AgentToolContent,
} from "../hooks/useAgentEvents";

interface AgentEventStreamProps {
  events: AgentEvent[];
  onConfirm: (confirmationId: string, accepted: boolean) => Promise<void>;
  expanded?: boolean;
  onToggleExpanded?: () => void;
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
  expanded = true,
  onToggleExpanded,
}: AgentEventStreamProps) {
  const eventStyle: Record<
    AgentEventKind,
    { card: string; type: string; title: string }
  > = {
    thinking: {
      card: "border-l-violet-500",
      type: "text-slate-500",
      title: "text-slate-900",
    },
    tool: {
      card: "border-l-sky-600",
      type: "text-slate-500",
      title: "text-slate-900",
    },
    result: {
      card: "border-l-emerald-600",
      type: "text-slate-500",
      title: "text-slate-900",
    },
    diff: {
      card: "border-l-amber-700",
      type: "text-slate-500",
      title: "text-slate-900",
    },
    confirm: {
      card: "border-amber-500/40 border-l-amber-600 bg-amber-50",
      type: "text-amber-800",
      title: "text-amber-900",
    },
    error: {
      card: "border-rose-500/35 border-l-rose-600",
      type: "text-rose-700",
      title: "text-rose-900",
    },
    status: {
      card: "border-l-slate-400 bg-slate-50",
      type: "text-slate-600",
      title: "text-slate-900",
    },
  };

  const hasCriticalEvent = events.some(
    (event) => event.kind === "confirm" || event.kind === "error",
  );
  const latestEvent = events.length > 0 ? events[events.length - 1] : undefined;

  if (!expanded) {
    return (
      <section
        className={`timeline-shell rounded-lg border bg-white p-3 ${
          hasCriticalEvent
            ? "border-amber-500/45"
            : "border-slate-200"
        }`}
        aria-label="Agent output timeline"
      >
        <div className="flex items-center justify-between gap-3 max-sm:flex-col max-sm:items-stretch">
          <div>
            <div className="text-[11px] font-extrabold uppercase text-slate-500">
              Timeline
            </div>
            <p className="mt-1 text-sm font-bold text-slate-900">
              {events.length === 0
                ? "No agent events yet"
                : `${events.length} event${events.length === 1 ? "" : "s"} stored`}
            </p>
            {latestEvent && (
              <p className="mt-1 max-h-10 overflow-hidden text-sm leading-5 text-slate-500">
                Latest: {eventLabels[latestEvent.kind]}{" "}
                {latestEvent.title
                  ? `- ${latestEvent.title}`
                  : displayAgentEventContent(latestEvent)}
              </p>
            )}
          </div>
          <button
            className="min-h-9 cursor-pointer rounded-lg border border-slate-300 px-3 text-sm font-extrabold text-slate-700 transition-colors duration-200 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
            onClick={onToggleExpanded}
            type="button"
          >
            Show timeline
          </button>
        </div>
      </section>
    );
  }

  if (events.length === 0) {
    return (
      <section
        className="timeline-shell flex min-h-[140px] flex-col justify-center rounded-lg border border-dashed border-slate-300 p-3"
        aria-label="Agent output timeline"
      >
        <div className="mb-2 flex items-center justify-between gap-3">
          <div className="text-[11px] font-extrabold uppercase text-slate-500">
            Timeline
          </div>
          {onToggleExpanded && (
            <button
              className="min-h-8 cursor-pointer rounded-lg border border-slate-300 px-2.5 text-xs font-extrabold text-slate-700 transition-colors duration-200 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
              onClick={onToggleExpanded}
              type="button"
            >
              Hide timeline
            </button>
          )}
        </div>
        <div className="text-center text-slate-500">Agent output will appear here.</div>
      </section>
    );
  }

  return (
    <section
      className="timeline-shell flex max-h-[42vh] min-h-[220px] flex-col gap-2.5 overflow-y-auto rounded-lg border border-slate-200 bg-white p-3"
      aria-label="Agent output timeline"
    >
      <div className="sticky top-0 z-10 flex items-center justify-between gap-3 bg-white/95 pb-1">
        <div>
          <div className="text-[11px] font-extrabold uppercase text-slate-500">
            Timeline
          </div>
          <p className="text-sm font-bold text-slate-900">Full agent history</p>
        </div>
        {onToggleExpanded && (
          <button
            className="min-h-8 cursor-pointer rounded-lg border border-slate-300 px-2.5 text-xs font-extrabold text-slate-700 transition-colors duration-200 hover:bg-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
            onClick={onToggleExpanded}
            type="button"
          >
            Hide timeline
          </button>
        )}
      </div>
      {events.map((event) => (
        <article
          className={`rounded-lg border border-slate-300 border-l-4 bg-white px-3 py-2.5 ${eventStyle[event.kind].card} ${
            event.tool?.status === "failed"
              ? "agent-event-tool-failed border-rose-500/35 border-l-rose-600"
              : ""
          }`}
          key={event.id}
        >
          <header className="mb-1.5 flex items-center gap-2">
            <span
              className={`text-[11px] font-black uppercase tracking-[0.06em] ${eventStyle[event.kind].type}`}
            >
              {eventLabels[event.kind]}
            </span>
            {event.title && (
              <span className={`text-xs font-extrabold ${eventStyle[event.kind].title}`}>
                {event.title}
              </span>
            )}
            {event.tool?.status && (
              <span
                className={`rounded-full px-1.5 py-0.5 text-[11px] font-black uppercase ${
                  event.tool.status === "failed"
                    ? "bg-rose-600/10 text-rose-700"
                    : "bg-slate-100 text-slate-500"
                }`}
              >
                {event.tool.status.replace("_", " ")}
              </span>
            )}
          </header>

          {event.kind === "tool" && event.tool ? (
            <ToolEvent event={event} />
          ) : (
            <div className="whitespace-pre-wrap leading-[1.45]">
              {displayAgentEventContent(event)}
            </div>
          )}

          {event.diff && <DiffBlock diff={event.diff} />}
          {event.terminal && (
            <div className="mt-2 inline-block rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all">
              Terminal: {event.terminal.terminalId}
            </div>
          )}
          {event.contentBlocks?.map((block, index) =>
            block.kind === "text" ? null : (
              <div
                className="mt-2 inline-block rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all"
                key={`${block.kind}-${index}`}
              >
                {block.summary}
              </div>
            ),
          )}

          {event.kind === "confirm" && event.confirmationId && (
            <div className="mt-2.5 flex items-center gap-2">
              <button
                className="min-h-9 cursor-pointer rounded-lg bg-emerald-600 px-3 text-sm font-extrabold text-white transition-colors duration-200 hover:bg-emerald-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-700 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={event.confirmStatus !== "pending"}
                onClick={() => onConfirm(event.confirmationId!, true)}
              >
                Confirm
              </button>
              <button
                className="min-h-9 cursor-pointer rounded-lg bg-rose-600 px-3 text-sm font-extrabold text-white transition-colors duration-200 hover:bg-rose-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-700 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={event.confirmStatus !== "pending"}
                onClick={() => onConfirm(event.confirmationId!, false)}
              >
                Reject
              </button>
              {event.confirmStatus && event.confirmStatus !== "pending" && (
                <span className="text-xs font-extrabold capitalize text-slate-500">
                  {event.confirmStatus}
                </span>
              )}
            </div>
          )}
        </article>
      ))}
    </section>
  );
}

export function stripTtsControlBlocks(content: string): string {
  let stripped = "";
  let searchFrom = 0;

  while (searchFrom < content.length) {
    const start = content.indexOf("<tts>", searchFrom);
    if (start === -1) {
      stripped += content.slice(searchFrom);
      break;
    }

    const innerStart = start + "<tts>".length;
    const end = content.indexOf("</tts>", innerStart);
    if (end === -1) {
      stripped += content.slice(searchFrom);
      break;
    }

    stripped += content.slice(searchFrom, start);
    searchFrom = end + "</tts>".length;
  }

  return stripped.trim();
}

function displayAgentEventContent(event: AgentEvent): string {
  if (event.kind !== "result") {
    return event.content;
  }
  return stripTtsControlBlocks(event.content);
}

function ToolEvent({ event }: { event: AgentEvent }) {
  const tool = event.tool!;

  return (
    <div>
      <div className="mb-2 flex flex-wrap gap-1.5">
        {tool.kind && (
          <span className="rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all">
            {tool.kind}
          </span>
        )}
        {tool.toolCallId && (
          <span className="rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all">
            {tool.toolCallId}
          </span>
        )}
      </div>

      {tool.locations && tool.locations.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1.5">
          {tool.locations.map((location) => (
            <span
              className="rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all"
              key={`${location.path}:${location.line ?? ""}`}
            >
              {location.path}
              {location.line ? `:${location.line}` : ""}
            </span>
          ))}
        </div>
      )}

      {tool.content && tool.content.length > 0 ? (
        <div className="grid gap-2">
          {tool.content.map((content, index) => (
            <ToolContentBlock content={content} key={index} />
          ))}
        </div>
      ) : (
        <div className="whitespace-pre-wrap leading-[1.45]">{event.content}</div>
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
      <div className="inline-block rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all">
        Terminal: {content.terminal.terminalId}
      </div>
    );
  }
  if (content.content?.text) {
    return <div className="whitespace-pre-wrap leading-[1.45]">{content.content.text}</div>;
  }
  return (
    <div className="inline-block rounded-md border border-slate-300 bg-slate-100 px-1.5 py-1 text-xs font-bold text-slate-500 break-all">
      {content.summary}
    </div>
  );
}

function DiffBlock({ diff }: { diff: AgentDiff }) {
  return (
    <div className="mt-2 grid gap-1.5 rounded-lg border border-amber-700/30 bg-amber-50 p-2">
      <div className="text-xs font-black text-amber-800 break-all">{diff.path}</div>
      {diff.oldText !== undefined && (
        <pre className="m-0 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-xs leading-[1.45] text-rose-700">
          {diff.oldText}
        </pre>
      )}
      <pre className="m-0 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-xs leading-[1.45] text-emerald-700">
        {diff.newText}
      </pre>
    </div>
  );
}
