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

  if (events.length === 0) {
    return (
      <section className="flex min-h-[220px] flex-1 items-center justify-center overflow-y-auto rounded-lg border border-dashed border-slate-300">
        <div className="text-center text-slate-500">Agent output will appear here.</div>
      </section>
    );
  }

  return (
    <section
      className="flex min-h-[220px] flex-1 flex-col gap-2.5 overflow-y-auto"
      aria-label="Agent output stream"
    >
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
            <div className="whitespace-pre-wrap leading-[1.45]">{event.content}</div>
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
                className="min-h-9 rounded-lg bg-emerald-600 px-3 text-sm font-extrabold text-white transition-colors hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-60"
                disabled={event.confirmStatus !== "pending"}
                onClick={() => onConfirm(event.confirmationId!, true)}
              >
                Confirm
              </button>
              <button
                className="min-h-9 rounded-lg bg-rose-600 px-3 text-sm font-extrabold text-white transition-colors hover:bg-rose-700 disabled:cursor-not-allowed disabled:opacity-60"
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
