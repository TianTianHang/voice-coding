import type {
  AgentDiff,
  AgentToolContent,
} from "../hooks/useAgentEvents";
import {
  timelineItemToAgentEventKind,
  type AgentTimelineItem,
} from "../hooks/useAgentStream";

interface AgentEventStreamProps {
  events: AgentTimelineItem[];
  onConfirm: (confirmationId: string, accepted: boolean) => Promise<void>;
  expanded?: boolean;
  fixedHeight?: boolean;
  onToggleExpanded?: () => void;
}

type DisplayEventKind =
  | "thinking"
  | "tool"
  | "result"
  | "diff"
  | "confirm"
  | "error"
  | "status";

const eventLabels: Record<DisplayEventKind, string> = {
  thinking: "思考中",
  tool: "工具",
  result: "结果",
  diff: "变更",
  confirm: "等待确认",
  error: "错误",
  status: "状态",
};

const eventDisplayContent = (event: AgentTimelineItem): string => {
  const kind = timelineItemToAgentEventKind(event.kind);
  if (kind === "thinking") {
    return event.content || "";
  }
  if (event.title && event.content) {
    return `${event.title} ${displayAgentEventContent(event)}`;
  }
  return event.title || displayAgentEventContent(event);
};

const eventPriority: Record<
  DisplayEventKind,
  { dot: string; line: string; icon: string }
> = {
  confirm: {
    dot: "border-rose-300 bg-rose-500 text-white",
    line: "bg-rose-400/30",
    icon: "!",
  },
  error: {
    dot: "border-rose-300 bg-rose-500 text-white",
    line: "bg-rose-400/30",
    icon: "!",
  },
  result: {
    dot: "border-emerald-300 bg-emerald-500 text-slate-950",
    line: "bg-emerald-400/25",
    icon: "✓",
  },
  diff: {
    dot: "border-violet-300 bg-violet-500 text-white",
    line: "bg-violet-400/25",
    icon: "✎",
  },
  tool: {
    dot: "border-violet-300 bg-violet-500 text-white",
    line: "bg-violet-400/25",
    icon: "□",
  },
  status: {
    dot: "border-sky-300 bg-sky-500 text-white",
    line: "bg-sky-400/20",
    icon: "⌕",
  },
  thinking: {
    dot: "border-amber-300 bg-amber-400 text-slate-950",
    line: "bg-amber-300/20",
    icon: "Ⅱ",
  },
};

export function AgentEventStream({
  events,
  onConfirm,
  expanded = true,
  fixedHeight = false,
  onToggleExpanded,
}: AgentEventStreamProps) {
  const hasCriticalEvent = events.some(
    (event) =>
      timelineItemToAgentEventKind(event.kind) === "confirm" ||
      timelineItemToAgentEventKind(event.kind) === "error",
  );
  const latestEvent = events.length > 0 ? events[events.length - 1] : undefined;

  if (!expanded) {
    return (
      <section
        className={`timeline-shell rounded-lg border bg-white/[0.03] px-3 py-2.5 ${
          hasCriticalEvent
            ? "border-rose-300/35"
            : "border-white/10"
        }`}
        aria-label="Agent output timeline"
      >
        <div className="flex items-center justify-between gap-3 max-sm:flex-col max-sm:items-stretch">
          <div>
            <div className="text-[10px] font-extrabold uppercase text-slate-400">
              轨迹
            </div>
            <p className="mt-0.5 text-sm font-bold text-slate-100">
              {events.length > 0 ? `已记录 ${events.length} 条事件` : ""}
            </p>
            {latestEvent && (
              <p className="mt-0.5 max-h-5 overflow-hidden text-xs leading-5 text-slate-400">
                最新：{eventLabels[timelineItemToAgentEventKind(latestEvent.kind)]}{" "}
                {latestEvent.title
                  ? `· ${latestEvent.title}`
                  : displayAgentEventContent(latestEvent)}
              </p>
            )}
          </div>
          <button
            className="min-h-8 cursor-pointer rounded-md border border-white/10 px-2.5 text-xs font-extrabold text-slate-200 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
            onClick={onToggleExpanded}
            type="button"
          >
            查看轨迹
          </button>
        </div>
      </section>
    );
  }

  if (events.length === 0) {
    return (
      <section
        className={`timeline-shell flex flex-col overflow-y-auto rounded-lg border border-dashed border-white/15 bg-white/[0.03] p-3 ${
          fixedHeight ? "h-[176px]" : "min-h-[96px]"
        }`}
        aria-label="Agent output timeline"
      >
        <div className="sticky top-0 z-10 bg-[#101720]/95 pb-2">
          <SectionHeading />
        </div>
        <div className="min-h-24" aria-hidden="true" />
      </section>
    );
  }

  return (
      <section
        className={`timeline-shell flex flex-col gap-1 overflow-y-auto rounded-lg border border-white/10 bg-white/[0.03] p-3 ${
          fixedHeight ? "h-[176px]" : "max-h-[220px] min-h-[190px]"
        }`}
        aria-label="Agent output timeline"
      >
      <div className="sticky top-0 z-10 flex items-center justify-between gap-3 bg-[#101720]/95 pb-2">
        <SectionHeading />
        {onToggleExpanded && (
          <button
            className="inline-flex min-h-7 cursor-pointer items-center gap-2 rounded-full px-2.5 text-xs font-bold text-slate-300 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
            onClick={onToggleExpanded}
            type="button"
          >
            自动跟随
            <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />
          </button>
        )}
      </div>
      {events.slice(-20).map((event, index, visibleEvents) => (
        <article className="grid grid-cols-[56px_22px_minmax(0,1fr)] gap-1.5" key={event.id}>
          <time className="pt-1.5 text-right text-xs tabular-nums text-slate-400">
            {formatEventTime(event.createdAt)}
          </time>
          <div className="relative flex justify-center">
            {index < visibleEvents.length - 1 && (
              <span className={`absolute top-6 bottom-[-0.25rem] w-px ${eventPriority[timelineItemToAgentEventKind(event.kind)].line}`} />
            )}
            <span className={`relative z-10 grid h-4 w-4 place-items-center rounded-full border text-[9px] font-black ${eventPriority[timelineItemToAgentEventKind(event.kind)].dot}`}>
              {eventPriority[timelineItemToAgentEventKind(event.kind)].icon}
            </span>
          </div>
          <div
            className={`timeline-event mb-1.5 rounded-lg text-slate-200 ${
              event.tool?.status === "failed"
                ? "agent-event-tool-failed text-rose-100"
                : ""
            }`}
          >
            <div className="flex flex-wrap items-baseline gap-2">
              {timelineItemToAgentEventKind(event.kind) === "thinking" ||
              timelineItemToAgentEventKind(event.kind) === "tool" ? (
                <span className="text-sm font-black text-slate-100">
                  {timelineItemToAgentEventKind(event.kind) === "tool" ? eventLabels.tool : event.title || eventLabels.thinking}
                </span>
              ) : null}
              <span className="whitespace-pre-wrap text-xs font-semibold leading-[1.45] text-slate-200">
                {eventDisplayContent(event)}
              </span>
            </div>

            {timelineItemToAgentEventKind(event.kind) === "tool" && event.tool && <ToolEventDetails event={event} />}
            {event.diff && <DiffBlock diff={event.diff} />}
            {event.terminal && (
              <div className="mt-2 inline-block rounded-md border border-white/10 bg-white/8 px-1.5 py-1 text-xs font-bold text-slate-300 break-all">
                Terminal: {event.terminal.terminalId}
              </div>
            )}
            {event.contentBlocks?.map((block, blockIndex) =>
              block.kind === "text" ? null : (
                <div
                  className="mt-2 inline-block rounded-md border border-white/10 bg-white/8 px-1.5 py-1 text-xs font-bold text-slate-300 break-all"
                  key={`${block.kind}-${blockIndex}`}
                >
                  {block.summary}
                </div>
              ),
            )}

            {timelineItemToAgentEventKind(event.kind) === "confirm" && event.confirmation && (
              <div className="mt-2.5 flex items-center gap-2">
                <button
                  className="min-h-9 cursor-pointer rounded-md bg-emerald-500 px-3 text-sm font-extrabold text-slate-950 transition-colors duration-200 hover:bg-emerald-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300 disabled:cursor-not-allowed disabled:opacity-60"
                  disabled={event.confirmation.status !== "pending"}
                  onClick={() => onConfirm(event.confirmation!.confirmationId, true)}
                >
                  确认
                </button>
                <button
                  className="min-h-9 cursor-pointer rounded-md bg-rose-500 px-3 text-sm font-extrabold text-white transition-colors duration-200 hover:bg-rose-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-300 disabled:cursor-not-allowed disabled:opacity-60"
                  disabled={event.confirmation.status !== "pending"}
                  onClick={() => onConfirm(event.confirmation!.confirmationId, false)}
                >
                  拒绝
                </button>
                {event.confirmation.status !== "pending" && (
                  <span className="text-xs font-extrabold capitalize text-slate-400">
                    {event.confirmation.status}
                  </span>
                )}
              </div>
            )}
          </div>
        </article>
      ))}
    </section>
  );
}

function ToolEventDetails({ event }: { event: AgentTimelineItem }) {
  const tool = event.tool!;
  const details = [
    tool.status,
    tool.kind,
    tool.toolCallId,
    ...(tool.locations?.map((location) =>
      location.line ? `${location.path}:${location.line}` : location.path,
    ) ?? []),
  ].filter(Boolean);

  return (
    <div className="mt-1.5 grid gap-1.5">
      {details.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {details.map((detail) => (
            <span
              className="rounded-md border border-white/10 bg-white/8 px-1.5 py-0.5 text-xs font-bold text-slate-300 break-all"
              key={detail}
            >
              {detail}
            </span>
          ))}
        </div>
      )}
      {tool.content?.map((content, index) => (
        <ToolContentBlock content={content} key={index} />
      ))}
    </div>
  );
}

function ToolContentBlock({ content }: { content: AgentToolContent }) {
  if (content.diff) {
    return <DiffBlock diff={content.diff} />;
  }
  if (content.terminal) {
    return (
      <div className="inline-block rounded-md border border-white/10 bg-white/8 px-1.5 py-1 text-xs font-bold text-slate-300 break-all">
        Terminal: {content.terminal.terminalId}
      </div>
    );
  }
  if (content.content?.text) {
    return <div className="whitespace-pre-wrap text-sm leading-[1.45]">{content.content.text}</div>;
  }
  return (
    <div className="inline-block rounded-md border border-white/10 bg-white/8 px-1.5 py-1 text-xs font-bold text-slate-300 break-all">
      {content.summary}
    </div>
  );
}

function SectionHeading() {
  return (
    <div className="flex items-center gap-3">
      <span className="text-slate-300" aria-hidden="true">
        <svg className="h-6 w-6" viewBox="0 0 24 24" fill="transparent" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M8 6h13" />
          <path d="M8 12h13" />
          <path d="M8 18h13" />
          <path d="M3 6h.01" />
          <path d="M3 12h.01" />
          <path d="M3 18h.01" />
        </svg>
      </span>
      <h2 className="m-0 text-base font-black text-slate-100">轨迹</h2>
    </div>
  );
}

function formatEventTime(createdAt: number): string {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) {
    return "--:--:--";
  }
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
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

function displayAgentEventContent(event: AgentTimelineItem): string {
  if (timelineItemToAgentEventKind(event.kind) !== "result") {
    return event.content;
  }
  return stripTtsControlBlocks(event.content);
}

function DiffBlock({ diff }: { diff: AgentDiff }) {
  return (
    <div className="mt-2 grid gap-1.5 rounded-lg border border-violet-300/20 bg-slate-950/35 p-2">
      <div className="text-xs font-black text-violet-200 break-all">{diff.path}</div>
      {diff.oldText !== undefined && (
        <pre className="m-0 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-xs leading-[1.45] text-rose-200">
          {diff.oldText}
        </pre>
      )}
      <pre className="m-0 max-h-56 overflow-auto whitespace-pre-wrap font-mono text-xs leading-[1.45] text-emerald-200">
        {diff.newText}
      </pre>
    </div>
  );
}
