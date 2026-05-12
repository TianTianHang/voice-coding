import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useBackendVAD, type VADState } from "../hooks/useBackendVAD";
import {
  asrStatusLabel,
  useAsrStatus,
  type ModelPathSnapshot,
} from "../hooks/useAsrStatus";
import {
  useAgentEvents,
  type AgentConnectionState,
  type AgentEvent,
} from "../hooks/useAgentEvents";
import { useBusinessApi, type SpeechOutputStatus } from "../hooks/useBusinessApi";
import { AgentEventStream } from "./AgentEventStream";
import { AudioVisualizer } from "./AudioVisualizer";

export type TtsStatusSnapshot = {
  state: "idle" | "synthesizing" | "ready" | "playing" | "failed";
  engineName: string;
  model: ModelPathSnapshot;
  error?: string;
  hasBufferedAudio: boolean;
};

export type AutoTtsStatusSnapshot = {
  enabled: boolean;
  isPlaying: boolean;
  latestResultText?: string;
  latestResultKey?: string;
  latestSpokenResultKey?: string;
  lastSkipReason?: string;
  lastStatus:
    | "idle"
    | "disabled"
    | "speaking"
    | "skippedDuplicate"
    | "skippedMissingTag"
    | "skippedInvalidTag"
    | "skippedEmptyTag"
    | "stopped"
    | "failed";
  tts: TtsStatusSnapshot;
};

export type VoiceExperienceState =
  | "Dormant"
  | "WakeDetected"
  | "Listening"
  | "Processing"
  | "Responding"
  | "Error";

type ConsoleView =
  | "main"
  | "status"
  | "transcript"
  | "response"
  | "events"
  | "settings";

type SettingsSection =
  | "general"
  | "speech"
  | "tts"
  | "display"
  | "shortcuts"
  | "about";

type ExperienceInput = {
  vadState: VADState;
  wakeDetected: boolean;
  speechError?: string | null;
  agentConnectionState: AgentConnectionState;
  latestAgentEvent?: AgentEvent;
};

const experienceCopy: Record<
  VoiceExperienceState,
  { headline: string; detail: string; status: string; intent: string }
> = {
  Dormant: {
    headline: "就绪",
    detail: "说出唤醒词，开始语音编码。",
    status: "等待语音唤醒",
    intent: "",
  },
  WakeDetected: {
    headline: "已唤醒",
    detail: "已检测到唤醒词，请继续说。",
    status: "唤醒已确认",
    intent: "准备收听",
  },
  Listening: {
    headline: "正在听",
    detail: "说话中...",
    status: "检测到你的声音",
    intent: "正在理解你的请求",
  },
  Processing: {
    headline: "处理中",
    detail: "正在转写、发送或等待 Agent。",
    status: "正在处理请求",
    intent: "把任务交给 Agent",
  },
  Responding: {
    headline: "播报中",
    detail: "正在展示 Agent 的最新回复。",
    status: "回复已生成",
    intent: "回答当前回合",
  },
  Error: {
    headline: "出错",
    detail: "需要处理错误后继续语音流程。",
    status: "需要关注",
    intent: "恢复会话",
  },
};

export function deriveVoiceExperienceState({
  vadState,
  wakeDetected,
  speechError,
  agentConnectionState,
  latestAgentEvent,
}: ExperienceInput): VoiceExperienceState {
  if (
    speechError ||
    agentConnectionState === "error" ||
    latestAgentEvent?.kind === "error"
  ) {
    return "Error";
  }

  if (wakeDetected) {
    return "WakeDetected";
  }

  if (latestAgentEvent?.kind === "result") {
    return "Responding";
  }

  if (
    vadState === "processing" ||
    latestAgentEvent?.kind === "thinking" ||
    latestAgentEvent?.kind === "tool" ||
    latestAgentEvent?.kind === "status"
  ) {
    return "Processing";
  }

  if (vadState === "listening" || vadState === "recording") {
    return "Listening";
  }

  return "Dormant";
}

function latestEventOfKind(events: AgentEvent[], kind: AgentEvent["kind"]) {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (events[index].kind === kind) {
      return events[index];
    }
  }
  return undefined;
}

export function shouldExpandTimelineForEvent(event?: AgentEvent): boolean {
  return event?.kind === "confirm" || event?.kind === "error";
}

export function autoTtsStatusLabel(status?: AutoTtsStatusSnapshot | null): string {
  if (!status) {
    return "";
  }
  if (!status.enabled) {
    return "自动播报关闭";
  }
  if (status.isPlaying || status.lastStatus === "speaking") {
    return "正在播报回复";
  }
  if (status.lastStatus === "skippedDuplicate") {
    return "重复回复已跳过";
  }
  if (
    status.lastStatus === "skippedMissingTag" ||
    status.lastStatus === "skippedInvalidTag" ||
    status.lastStatus === "skippedEmptyTag"
  ) {
    return "自动播报已跳过";
  }
  if (status.lastStatus === "failed") {
    return status.tts.error ? `自动播报失败：${status.tts.error}` : "自动播报失败";
  }
  return "自动播报已开启";
}

function emptyTtsModelSnapshot(): ModelPathSnapshot {
  return {
    kind: "tts",
    modelId: "",
    engineName: "",
    packageDir: "",
    modelDir: "",
    source: "devFallback",
    legacyLayout: false,
    missingFiles: [],
  };
}

function isTtsStatusSnapshot(value: unknown): value is TtsStatusSnapshot {
  return (
    typeof value === "object" &&
    value !== null &&
    "state" in value &&
    "engineName" in value &&
    "model" in value &&
    "hasBufferedAudio" in value
  );
}

function ttsStatusFromSpeech(
  speech: SpeechOutputStatus,
  value: unknown,
): TtsStatusSnapshot {
  if (isTtsStatusSnapshot(value)) {
    return value;
  }

  return {
    state:
      speech.state === "synthesizing"
        ? "synthesizing"
        : speech.state === "ready"
          ? "ready"
          : speech.state === "playing"
            ? "playing"
            : speech.state === "failed"
              ? "failed"
              : "idle",
    engineName: "",
    model: emptyTtsModelSnapshot(),
    error: speech.error,
    hasBufferedAudio: speech.state === "ready" || speech.state === "playing",
  };
}

function autoTtsStatusFromBusiness(
  speech?: SpeechOutputStatus,
  tts?: unknown,
): AutoTtsStatusSnapshot | null {
  if (!speech) {
    return null;
  }

  return {
    enabled: speech.autoSpeakAgentResults,
    isPlaying: speech.state === "playing",
    lastStatus: !speech.autoSpeakAgentResults
      ? "disabled"
      : speech.state === "playing"
        ? "speaking"
        : speech.state === "stopping"
          ? "stopped"
          : speech.state === "failed"
            ? "failed"
            : "idle",
    tts: ttsStatusFromSpeech(speech, tts),
  };
}

export function AssistantConsole() {
  const [closeBehavior, setCloseBehaviorState] = useState<"hide" | "exit">(
    "hide",
  );
  const [wakeDetected, setWakeDetected] = useState(false);
  const [activeView, setActiveView] = useState<ConsoleView>("main");
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("general");
  const [draftTranscript, setDraftTranscript] = useState("");
  const [autoTtsStatus, setAutoTtsStatus] = useState<AutoTtsStatusSnapshot | null>(null);
  const [debugWindowMessage, setDebugWindowMessage] = useState<string | null>(null);
  const {
    state,
    transcript,
    error,
    recordingDuration,
    startListening,
    stopListening,
  } = useBackendVAD();
  const { status: asrStatus, error: asrStatusError } = useAsrStatus();
  const agent = useAgentEvents();
  const business = useBusinessApi();
  const latestAgentEvent =
    agent.events.length > 0 ? agent.events[agent.events.length - 1] : undefined;

  useEffect(() => {
    const status = autoTtsStatusFromBusiness(
      business.status?.speech,
      business.status?.tts,
    );
    if (!status) {
      return;
    }
    setAutoTtsStatus((current) => ({ ...current, ...status }));
  }, [business.status?.speech, business.status?.tts]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    async function setupAutoTtsEvents() {
      unlisten = await listen<AutoTtsStatusSnapshot>("auto-tts-state", (event) => {
        setAutoTtsStatus(event.payload);
      });
    }

    void setupAutoTtsEvents();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const speechError = error
    ? error.includes("denied") || error.includes("NotAllowedError")
      ? `${error} Please grant microphone permission in system settings.`
      : error
    : asrStatusError;

  useEffect(() => {
    if (state !== "recording") {
      return;
    }

    setWakeDetected(true);
    const timeout = window.setTimeout(() => {
      setWakeDetected(false);
    }, 900);
    return () => window.clearTimeout(timeout);
  }, [state]);

  useEffect(() => {
    setDraftTranscript(transcript || "");
  }, [transcript]);

  const experienceState = deriveVoiceExperienceState({
    vadState: state,
    wakeDetected,
    speechError,
    agentConnectionState: agent.connectionState,
    latestAgentEvent,
  });
  const copy = experienceCopy[experienceState];

  const feedback = useMemo(() => {
    const responseEvent = latestEventOfKind(agent.events, "result");
    const thinkingEvent = latestEventOfKind(agent.events, "thinking");
    const toolEvent = latestEventOfKind(agent.events, "tool");
    const statusEvent = latestEventOfKind(agent.events, "status");
    const errorEvent = latestEventOfKind(agent.events, "error");

    return {
      heard: speechError || transcript || "",
      intent:
        thinkingEvent?.content ||
        toolEvent?.title ||
        toolEvent?.content ||
        copy.intent,
      status:
        speechError ||
        errorEvent?.content ||
        statusEvent?.content ||
        copy.status,
      response: responseEvent?.content || "",
    };
  }, [agent.events, copy.intent, copy.status, speechError, transcript]);

  async function setCloseBehavior(behavior: "hide" | "exit") {
    setCloseBehaviorState(behavior);
    try {
      await invoke("set_close_behavior", { behavior });
    } catch {
      setCloseBehaviorState(closeBehavior);
    }
  }

  async function openDebugToolsWindow() {
    setDebugWindowMessage(null);
    try {
      const existing = await WebviewWindow.getByLabel("debug-tools");
      if (existing) {
        await existing.show();
        await existing.setFocus();
        return;
      }

      const debugWindow = new WebviewWindow("debug-tools", {
        title: "Voice Debug Tools",
        url: "/?window=debug-tools",
        width: 520,
        height: 700,
        minWidth: 380,
        minHeight: 560,
        center: true,
        focus: true,
      });

      debugWindow.once("tauri://error", (event) => {
        setDebugWindowMessage(String(event.payload));
      });
    } catch (error) {
      setDebugWindowMessage(String(error));
    }
  }

  async function minimizeWindow() {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      setDebugWindowMessage(String(error));
    }
  }

  async function toggleMaximizeWindow() {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (error) {
      setDebugWindowMessage(String(error));
    }
  }

  async function closeWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      setDebugWindowMessage(String(error));
    }
  }

  const responseEvent = latestEventOfKind(agent.events, "result");
  const selectedEvent = latestAgentEvent ?? agent.events[agent.events.length - 1];

  if (activeView !== "main") {
    return (
      <main className="voice-shell min-h-screen text-slate-100">
      <section className="voice-console mx-auto flex min-h-screen w-full max-w-[420px] flex-col gap-2 border border-white/10 p-2.5">
          <SecondaryHeader
            title={secondaryTitle(activeView)}
            subtitle={secondarySubtitle(activeView, copy.status)}
            onBack={() => setActiveView("main")}
            onClose={closeWindow}
            onMaximize={toggleMaximizeWindow}
            onMinimize={minimizeWindow}
            onSettings={() => setActiveView("settings")}
          />

          {activeView === "status" && (
            <StatusDetailPage
              copy={copy}
              experienceState={experienceState}
              state={state}
              recordingDuration={recordingDuration}
              asrStatus={asrStatus}
              asrStatusError={asrStatusError}
              agentConnectionLabel={agent.connectionLabel}
              agentConnectionState={agent.connectionState}
              autoTtsStatus={autoTtsStatus}
              speechError={speechError}
              onStart={startListening}
              onStop={stopListening}
            />
          )}

          {activeView === "transcript" && (
            <TranscriptDetailPage
              value={draftTranscript}
              savedValue={transcript || ""}
              intent={feedback.intent}
              status={feedback.status}
              onChange={setDraftTranscript}
            />
          )}

          {activeView === "response" && (
            <ResponseDetailPage
              response={feedback.response}
              event={responseEvent}
              plan={agent.plan}
              onOpenEvents={() => setActiveView("events")}
            />
          )}

          {activeView === "events" && (
            <EventDetailPage
              events={agent.events}
              selectedEvent={selectedEvent}
              plan={agent.plan}
              onConfirm={agent.respondToConfirmation}
            />
          )}

          {activeView === "settings" && (
            <SettingsPage
              activeSection={settingsSection}
              asrStatus={asrStatus}
              autoTtsStatus={autoTtsStatus}
              closeBehavior={closeBehavior}
              onCloseBehaviorChange={setCloseBehavior}
              onSectionChange={setSettingsSection}
              onOpenDebugTools={openDebugToolsWindow}
            />
          )}

          {debugWindowMessage && (
            <p className="text-xs font-semibold text-rose-300">{debugWindowMessage}</p>
          )}
        </section>
      </main>
    );
  }

  return (
    <main className="voice-shell min-h-screen text-slate-100">
      <section className="voice-console mx-auto flex min-h-screen w-full max-w-[420px] flex-col gap-1.5 border border-white/10 p-2.5">
        <header className="app-titlebar flex items-center justify-between gap-1.5 px-0.5 pb-1.5" data-tauri-drag-region>
          <div className="flex min-w-0 flex-1 items-center gap-2" data-tauri-drag-region>
            <div className="grid h-7 w-7 place-items-center rounded-md bg-emerald-200 text-slate-950 shadow-[0_0_22px_rgba(49,232,138,0.25)]" aria-hidden="true">
              <MicGlyph />
            </div>
            <h1 className="m-0 truncate text-base font-black leading-tight text-slate-50" data-tauri-drag-region>语音编码助手</h1>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <IconButton label="固定窗口">
              <PinGlyph />
            </IconButton>
            <IconButton label="设置" onClick={() => setActiveView("settings")}>
              <CogGlyph />
            </IconButton>
            <WindowControls
              onClose={closeWindow}
              onMaximize={toggleMaximizeWindow}
              onMinimize={minimizeWindow}
            />
          </div>
        </header>

        <section className="voice-workbench" aria-label="Voice first workspace">
          <section
            className={`presence-stage presence-stage-${experienceState.toLowerCase()}`}
            aria-label="Voice presence stage"
          >
            <div className="presence-header">
              <div className="presence-orb-wrap" aria-hidden="true">
                <div className={`presence-orb presence-orb-${experienceState.toLowerCase()}`}>
                  <span className="presence-bar" />
                  <span className="presence-bar" />
                  <span className="presence-bar" />
                  <span className="presence-bar" />
                  <span className="presence-bar" />
                </div>
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="text-3xl font-black leading-none text-emerald-300 max-sm:text-3xl">
                  {copy.headline}
                </h2>
                <p className="mt-2 text-base font-bold text-slate-200">{copy.detail}</p>
                <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
                  <AudioVisualizer state={state} recordingDuration={recordingDuration} />
                  <button
                    className="min-h-9 cursor-pointer rounded-md px-2 text-sm font-extrabold text-emerald-300 transition-colors duration-200 hover:bg-emerald-400/10 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300 disabled:cursor-not-allowed disabled:text-slate-500"
                    onClick={state === "idle" ? startListening : stopListening}
                    disabled={state === "processing"}
                    type="button"
                  >
                    {state === "idle" ? "点击开始" : state === "processing" ? "处理中" : "点击停止"}
                  </button>
                </div>
              </div>
            </div>
          </section>
        </section>

        <section className="grid gap-1.5 px-0.5" aria-label="Voice feedback loop">
          <section className="heard-panel" aria-label="Recognized speech">
            <div className="flex items-center justify-between gap-3">
              <SectionTitle icon={<ChatGlyph />} title="我听到" />
              <div className="flex shrink-0 items-center gap-1">
                <button
                  className="min-h-7 cursor-pointer rounded-full border border-emerald-300/25 px-2.5 text-[11px] font-extrabold text-emerald-300 transition-colors duration-200 hover:bg-emerald-400/10 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300 disabled:cursor-not-allowed disabled:opacity-60"
                  onClick={agent.connectionState === "connected" ? agent.disconnect : agent.connect}
                  disabled={agent.connectionState === "connecting"}
                  type="button"
                >
                  {agent.connectionState === "connected" ? "断开" : "连接"}
                </button>
                <button
                  className="grid h-8 w-8 cursor-pointer place-items-center rounded-md text-slate-400 transition-colors duration-200 hover:bg-white/8 hover:text-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
                  aria-label="编辑识别结果"
                  onClick={() => setActiveView("transcript")}
                  type="button"
                >
                  <EditGlyph />
                </button>
              </div>
            </div>
            <p className={`mt-1.5 min-h-6 text-sm font-bold leading-6 ${speechError ? "text-rose-300" : "text-slate-100"}`}>
              {feedback.heard}
            </p>
          </section>

          <section className="feedback-tile feedback-tile-featured rounded-lg border p-3" aria-label="Current response">
            <SectionTitle icon={<SparkGlyph />} title="当前回复" accent="violet" />
            <div className="mt-2 max-h-[4.25rem] min-h-6 overflow-hidden border-l-2 border-violet-400/70 pl-3 text-sm font-semibold leading-6 text-slate-100">
              {feedback.response}
            </div>
            <div className="mt-2 flex items-center justify-between gap-3">
              <div className="flex items-center gap-3 text-slate-400" aria-hidden="true">
                <ThumbGlyph />
                <ThumbDownGlyph />
              </div>
              <button
                className="inline-flex min-h-8 cursor-pointer items-center gap-1.5 rounded-md px-1.5 text-xs font-extrabold text-sky-300 transition-colors duration-200 hover:bg-sky-400/10 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-300"
                onClick={() => setActiveView("response")}
                type="button"
              >
                打开完整回复
                <ExternalGlyph />
              </button>
            </div>
          </section>
        </section>

        {agent.plan && agent.plan.entries.length > 0 && (
          <section className="rounded-lg border border-white/10 bg-white/[0.03] p-3" aria-label="Agent plan">
            <div className="block text-[11px] font-extrabold uppercase text-slate-400">当前计划</div>
            <ol className="mt-2 grid gap-1.5 pl-5 text-sm">
              {agent.plan.entries.map((entry, index) => (
                <li
                  className={entry.status === "completed" ? "text-slate-500 line-through" : "text-slate-100"}
                  key={index}
                >
                  <span className="block">{entry.content}</span>
                  <small className="mt-0.5 block text-[11px] font-extrabold capitalize text-slate-400">
                    {entry.priority} · {entry.status.replace("_", " ")}
                  </small>
                </li>
              ))}
            </ol>
          </section>
        )}

        <AgentEventStream
          events={agent.events}
          expanded
          fixedHeight
          onConfirm={agent.respondToConfirmation}
          onToggleExpanded={() => {
            if (agent.events.length > 0) {
              setActiveView("events");
            }
          }}
        />

        <section className="session-strip text-sm" aria-label="Agent session">
          <FooterStatus value={asrStatus.state === "failed" ? asrStatusLabel(asrStatus) : "语音模型已就绪"} />
          <FooterStatus value={agent.connectionState === "connected" ? "Agent 已连接" : agent.connectionLabel} />
          <FooterStatus value={autoTtsStatus?.enabled ? "自动播报已开启" : "自动播报未开启"} />
        </section>
        {debugWindowMessage && (
          <p className="text-xs font-semibold text-rose-300">{debugWindowMessage}</p>
        )}
      </section>
    </main>
  );
}

function SectionTitle({
  icon,
  title,
  accent = "slate",
}: {
  icon: ReactNode;
  title: string;
  accent?: "slate" | "violet";
}) {
  return (
    <div className="flex items-center gap-2">
      <span className={accent === "violet" ? "text-violet-400" : "text-slate-300"} aria-hidden="true">
        {icon}
      </span>
      <h2 className="m-0 text-base font-black text-slate-100">{title}</h2>
    </div>
  );
}

function IconButton({
  children,
  label,
  onClick,
}: {
  children: ReactNode;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button
      className="grid h-8 w-8 cursor-pointer place-items-center rounded-md text-slate-400 transition-colors duration-200 hover:bg-white/8 hover:text-slate-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
      aria-label={label}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}

function FooterStatus({ value }: { value: string }) {
  return (
    <div className="flex min-w-0 items-center gap-2 text-slate-300">
      <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />
      <strong className="truncate text-xs">{value}</strong>
    </div>
  );
}

function secondaryTitle(view: ConsoleView): string {
  switch (view) {
    case "status":
      return "语音详情";
    case "transcript":
      return "识别编辑";
    case "response":
      return "完整回复";
    case "events":
      return "事件轨迹";
    case "settings":
      return "设置";
    case "main":
      return "语音编码助手";
  }
}

function secondarySubtitle(view: ConsoleView, status: string): string {
  switch (view) {
    case "status":
      return status;
    case "transcript":
      return "校正识别文本并确认意图";
    case "response":
      return "查看 Agent 回复、文件引用与执行计划";
    case "events":
      return "跟踪 Agent 的思考、工具调用与确认请求";
    case "settings":
      return "调整通用、语音、播报、显示与快捷键";
    case "main":
      return "";
  }
}

function SecondaryHeader({
  title,
  subtitle,
  onBack,
  onClose,
  onMaximize,
  onMinimize,
  onSettings,
}: {
  title: string;
  subtitle: string;
  onBack: () => void;
  onClose: () => void;
  onMaximize: () => void;
  onMinimize: () => void;
  onSettings: () => void;
}) {
  return (
    <header className="app-titlebar flex items-center justify-between gap-1.5 border-b border-white/8 pb-2" data-tauri-drag-region>
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <IconButton label="返回" onClick={onBack}>
          <BackGlyph />
        </IconButton>
        <div className="min-w-0 flex-1" data-tauri-drag-region>
          <h1 className="m-0 truncate text-base font-black text-slate-50">{title}</h1>
          <p className="mt-0.5 truncate text-[11px] font-semibold text-slate-400">{subtitle}</p>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <IconButton label="固定窗口">
          <PinGlyph />
        </IconButton>
        <IconButton label="设置" onClick={onSettings}>
          <CogGlyph />
        </IconButton>
        <WindowControls
          onClose={onClose}
          onMaximize={onMaximize}
          onMinimize={onMinimize}
        />
      </div>
    </header>
  );
}

function WindowControls({
  onClose,
  onMaximize,
  onMinimize,
}: {
  onClose: () => void;
  onMaximize: () => void;
  onMinimize: () => void;
}) {
  return (
    <div className="window-controls flex items-center gap-1" aria-label="窗口控制">
      <WindowButton label="最小化" onClick={onMinimize}>
        <MinimizeGlyph />
      </WindowButton>
      <WindowButton label="最大化或还原" onClick={onMaximize}>
        <MaximizeGlyph />
      </WindowButton>
      <WindowButton label="关闭窗口" onClick={onClose} danger>
        <CloseGlyph />
      </WindowButton>
    </div>
  );
}

function WindowButton({
  children,
  danger = false,
  label,
  onClick,
}: {
  children: ReactNode;
  danger?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      className={`grid h-7 w-7 cursor-pointer place-items-center rounded-md transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100 ${
        danger
          ? "text-slate-400 hover:bg-rose-500 hover:text-white"
          : "text-slate-400 hover:bg-white/8 hover:text-slate-100"
      }`}
      aria-label={label}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}

function StatusDetailPage({
  copy,
  experienceState,
  state,
  recordingDuration,
  asrStatus,
  asrStatusError,
  agentConnectionLabel,
  agentConnectionState,
  autoTtsStatus,
  speechError,
  onStart,
  onStop,
}: {
  copy: { headline: string; detail: string; status: string; intent: string };
  experienceState: VoiceExperienceState;
  state: VADState;
  recordingDuration: number;
  asrStatus: ReturnType<typeof useAsrStatus>["status"];
  asrStatusError: string | null;
  agentConnectionLabel: string;
  agentConnectionState: AgentConnectionState;
  autoTtsStatus?: AutoTtsStatusSnapshot | null;
  speechError?: string | null;
  onStart: () => void;
  onStop: () => void;
}) {
  const details = [
    { label: "语音状态", value: copy.status },
    { label: "识别模型", value: asrStatusLabel(asrStatus) },
    { label: "Agent", value: agentConnectionLabel },
    { label: "自动播报", value: autoTtsStatusLabel(autoTtsStatus) },
  ];

  return (
    <section className="secondary-grid" aria-label="Voice status detail">
      <div className="presence-stage">
        <div className="presence-header">
          <div className="presence-orb-wrap" aria-hidden="true">
            <div className={`presence-orb presence-orb-${experienceState.toLowerCase()}`}>
              <span className="presence-bar" />
              <span className="presence-bar" />
              <span className="presence-bar" />
              <span className="presence-bar" />
              <span className="presence-bar" />
            </div>
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-3xl font-black leading-none text-emerald-300">
              {copy.headline}
            </h2>
            <p className="mt-2 text-sm font-bold text-slate-200">{copy.detail}</p>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <AudioVisualizer state={state} recordingDuration={recordingDuration} />
              <button
                className="min-h-9 cursor-pointer rounded-md bg-emerald-500 px-3 text-sm font-black text-slate-950 transition-colors duration-200 hover:bg-emerald-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300 disabled:cursor-not-allowed disabled:bg-slate-600 disabled:text-slate-300"
                onClick={state === "idle" ? onStart : onStop}
                disabled={state === "processing"}
                type="button"
              >
                {state === "idle" ? "开始监听" : state === "processing" ? "处理中" : "停止监听"}
              </button>
            </div>
          </div>
        </div>
      </div>
      <div className="secondary-panel">
        <SectionTitle icon={<InfoGlyph />} title="状态说明" />
        {(speechError || asrStatusError) && (
          <p className="mt-3 text-sm font-semibold leading-6 text-rose-300">
            {speechError || asrStatusError}
          </p>
        )}
        <div className="mt-4 grid gap-2">
          {details.map((item) => (
            <KeyValueRow key={item.label} label={item.label} value={item.value} />
          ))}
          <KeyValueRow
            label="连接态"
            value={agentConnectionState === "connected" ? "已连接" : agentConnectionLabel}
          />
        </div>
      </div>
      <div className="secondary-panel">
        <SectionTitle icon={<SparkGlyph />} title="语音输入技巧" />
        <ul className="mt-4 grid gap-3 text-sm font-semibold leading-6 text-slate-300">
          <li>尽量先说目标，再说文件或组件名。</li>
          <li>需要精确改动时，说出函数、测试或报错关键词。</li>
          <li>如果识别偏差，进入识别编辑页修正文本。</li>
        </ul>
      </div>
    </section>
  );
}

function TranscriptDetailPage({
  value,
  savedValue,
  intent,
  status,
  onChange,
}: {
  value: string;
  savedValue: string;
  intent: string;
  status: string;
  onChange: (value: string) => void;
}) {
  return (
    <section className="secondary-grid" aria-label="Transcript editor">
      <div className="secondary-panel secondary-panel-tall">
        <SectionTitle icon={<ChatGlyph />} title="识别结果" />
        <textarea
          className="mt-4 min-h-[170px] w-full resize-none rounded-md border border-white/10 bg-slate-950/50 p-3 text-base font-semibold leading-7 text-slate-100 outline-none transition-colors duration-200 placeholder:text-slate-500 focus:border-emerald-300/70"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder=""
        />
        <div className="mt-4 grid grid-cols-2 gap-2 max-sm:grid-cols-1">
          <button className="min-h-10 cursor-pointer rounded-md border border-white/10 px-3 text-sm font-black text-slate-300 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100" type="button">
            取消
          </button>
          <button className="min-h-10 cursor-pointer rounded-md bg-emerald-500 px-3 text-sm font-black text-slate-950 transition-colors duration-200 hover:bg-emerald-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300" type="button">
            保存并确认
          </button>
        </div>
      </div>
      <div className="secondary-panel">
        <SectionTitle icon={<InfoGlyph />} title="信息识别结果" />
        <div className="mt-4 grid gap-3">
          <KeyValueRow label="原始识别" value={savedValue} />
          <KeyValueRow label="推断意图" value={intent} />
          <KeyValueRow label="当前状态" value={status} />
        </div>
      </div>
    </section>
  );
}

function ResponseDetailPage({
  response,
  event,
  plan,
  onOpenEvents,
}: {
  response: string;
  event?: AgentEvent;
  plan?: { entries: { content: string; priority: string; status: string }[] };
  onOpenEvents: () => void;
}) {
  const files = event?.tool?.locations ?? [];

  return (
    <section className="secondary-grid" aria-label="Full response">
      <article className="secondary-panel secondary-panel-tall">
        <SectionTitle icon={<SparkGlyph />} title="当前回复" accent="violet" />
        <div className="mt-4 rounded-md border border-violet-300/20 bg-violet-500/10 p-4 text-sm font-semibold leading-7 text-slate-100">
          {response}
        </div>
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3 text-slate-400" aria-hidden="true">
            <ThumbGlyph />
            <ThumbDownGlyph />
          </div>
          <button
            className="min-h-9 cursor-pointer rounded-md border border-white/10 px-3 text-sm font-black text-slate-300 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
            onClick={onOpenEvents}
            type="button"
          >
            查看事件轨迹
          </button>
        </div>
      </article>
      <div className="secondary-panel">
        <SectionTitle icon={<FileGlyph />} title="相关文件" />
        <div className="mt-4 grid gap-2">
          {files.length > 0 ? (
            files.map((file) => (
              <KeyValueRow
                key={`${file.path}:${file.line ?? 0}`}
                label={file.line ? `第 ${file.line} 行` : "文件"}
                value={file.path}
              />
            ))
          ) : null}
        </div>
      </div>
      <div className="secondary-panel">
        <SectionTitle icon={<ListGlyph />} title="执行计划" />
        {plan && plan.entries.length > 0 ? (
          <ol className="mt-4 grid gap-3 text-sm">
            {plan.entries.map((entry, index) => (
              <li className="rounded-md border border-white/10 bg-white/[0.03] p-3" key={`${entry.content}-${index}`}>
                <strong className="block text-slate-100">{entry.content}</strong>
                <span className="mt-1 block text-xs font-bold text-slate-400">
                  {entry.priority} · {entry.status.replace("_", " ")}
                </span>
              </li>
            ))}
          </ol>
        ) : null}
      </div>
    </section>
  );
}

function EventDetailPage({
  events,
  selectedEvent,
  plan,
  onConfirm,
}: {
  events: AgentEvent[];
  selectedEvent?: AgentEvent;
  plan?: { entries: { content: string; priority: string; status: string }[] };
  onConfirm: (confirmationId: string, accepted: boolean) => Promise<void>;
}) {
  return (
    <section className="secondary-grid" aria-label="Agent event detail">
      <AgentEventStream events={events} expanded onConfirm={onConfirm} />
      <aside className="secondary-panel">
        <SectionTitle icon={<InfoGlyph />} title="事件细节" />
        {selectedEvent ? (
          <div className="mt-4 grid gap-3">
            <KeyValueRow label="类型" value={selectedEvent.kind} />
            {selectedEvent.title && <KeyValueRow label="标题" value={selectedEvent.title} />}
            {selectedEvent.content && <KeyValueRow label="内容" value={selectedEvent.content} />}
            {selectedEvent.confirmationId && (
              <div className="grid grid-cols-2 gap-2">
                <button
                  className="min-h-10 cursor-pointer rounded-md bg-emerald-500 px-3 text-sm font-black text-slate-950 transition-colors duration-200 hover:bg-emerald-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300"
                  onClick={() => void onConfirm(selectedEvent.confirmationId!, true)}
                  type="button"
                >
                  确认
                </button>
                <button
                  className="min-h-10 cursor-pointer rounded-md border border-white/10 px-3 text-sm font-black text-slate-300 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
                  onClick={() => void onConfirm(selectedEvent.confirmationId!, false)}
                  type="button"
                >
                  取消
                </button>
              </div>
            )}
          </div>
        ) : null}
      </aside>
      <aside className="secondary-panel">
        <SectionTitle icon={<ListGlyph />} title="思考中" />
        {plan && plan.entries.length > 0 ? (
          <ul className="mt-4 grid gap-2 text-sm font-semibold text-slate-300">
            {plan.entries.map((entry, index) => (
              <li className="flex items-start gap-2" key={`${entry.content}-${index}`}>
                <span className="mt-1 h-2 w-2 rounded-full bg-emerald-400" />
                <span>{entry.content}</span>
              </li>
            ))}
          </ul>
        ) : null}
      </aside>
    </section>
  );
}

function SettingsPage({
  activeSection,
  asrStatus,
  autoTtsStatus,
  closeBehavior,
  onCloseBehaviorChange,
  onSectionChange,
  onOpenDebugTools,
}: {
  activeSection: SettingsSection;
  asrStatus: ReturnType<typeof useAsrStatus>["status"];
  autoTtsStatus?: AutoTtsStatusSnapshot | null;
  closeBehavior: "hide" | "exit";
  onCloseBehaviorChange: (behavior: "hide" | "exit") => void;
  onSectionChange: (section: SettingsSection) => void;
  onOpenDebugTools: () => void;
}) {
  const sections: { id: SettingsSection; label: string }[] = [
    { id: "general", label: "通用" },
    { id: "speech", label: "语音" },
    { id: "tts", label: "播报" },
    { id: "display", label: "显示" },
    { id: "shortcuts", label: "快捷键" },
    { id: "about", label: "关于" },
  ];

  return (
    <section className="settings-layout" aria-label="Settings">
      <nav className="settings-nav" aria-label="Settings sections">
        {sections.map((section) => (
          <button
            className={`settings-nav-item ${activeSection === section.id ? "settings-nav-item-active" : ""}`}
            key={section.id}
            onClick={() => onSectionChange(section.id)}
            type="button"
          >
            {section.label}
          </button>
        ))}
      </nav>
      <div className="secondary-panel">
        {activeSection === "general" && (
          <SettingsSectionPanel title="通用设置">
            <SettingToggle label="开机启动" checked />
            <SettingToggle label="最小化后保持后台运行" checked />
            <div className="mt-4">
              <span className="block text-xs font-black text-slate-400">关闭窗口</span>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <SegmentButton active={closeBehavior === "hide"} onClick={() => onCloseBehaviorChange("hide")}>
                  隐藏
                </SegmentButton>
                <SegmentButton active={closeBehavior === "exit"} onClick={() => onCloseBehaviorChange("exit")}>
                  退出
                </SegmentButton>
              </div>
            </div>
          </SettingsSectionPanel>
        )}
        {activeSection === "speech" && (
          <SettingsSectionPanel title="语音设置">
            <KeyValueRow label="ASR 引擎" value={asrStatus.engineName} />
            {(asrStatus.modelDir || asrStatus.model.modelDir) && (
              <KeyValueRow label="模型目录" value={asrStatus.modelDir || asrStatus.model.modelDir} />
            )}
            <SettingToggle label="静音检测" checked />
            <SettingToggle label="自动提交识别文本" checked />
          </SettingsSectionPanel>
        )}
        {activeSection === "tts" && (
          <SettingsSectionPanel title="播报设置">
            {autoTtsStatus?.tts.engineName && <KeyValueRow label="TTS 引擎" value={autoTtsStatus.tts.engineName} />}
            <KeyValueRow label="当前状态" value={autoTtsStatusLabel(autoTtsStatus)} />
            <SettingToggle label="自动播报 Agent 回复" checked={Boolean(autoTtsStatus?.enabled)} />
            <SettingToggle label="跳过重复回复" checked />
          </SettingsSectionPanel>
        )}
        {activeSection === "display" && (
          <SettingsSectionPanel title="显示设置">
            <KeyValueRow label="界面主题" value="深色模式" />
            <SettingToggle label="自动展开确认事件" checked />
            <SettingToggle label="显示时间轴细节" checked />
          </SettingsSectionPanel>
        )}
        {activeSection === "shortcuts" && (
          <SettingsSectionPanel title="快捷键设置">
            <KeyValueRow label="开始 / 停止监听" value="Alt + Space" />
            <KeyValueRow label="确认请求" value="Ctrl + Enter" />
            <KeyValueRow label="打开设置" value="Ctrl + ," />
            <button
              className="mt-4 min-h-10 w-full cursor-pointer rounded-md border border-white/10 px-3 text-sm font-black text-slate-300 transition-colors duration-200 hover:bg-white/8 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100"
              type="button"
            >
              重置快捷键
            </button>
          </SettingsSectionPanel>
        )}
        {activeSection === "about" && (
          <SettingsSectionPanel title="语音编码助手">
            <button
              className="mt-4 min-h-10 w-full cursor-pointer rounded-md bg-emerald-500 px-3 text-sm font-black text-slate-950 transition-colors duration-200 hover:bg-emerald-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-emerald-300"
              onClick={onOpenDebugTools}
              type="button"
            >
              打开调试工具
            </button>
          </SettingsSectionPanel>
        )}
      </div>
    </section>
  );
}

function SettingsSectionPanel({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section>
      <h2 className="m-0 text-lg font-black text-slate-100">{title}</h2>
      <div className="mt-4 grid gap-2.5">{children}</div>
    </section>
  );
}

function KeyValueRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-white/10 bg-white/[0.03] p-2.5">
      <span className="block text-[11px] font-black uppercase text-slate-500">{label}</span>
      <strong className="mt-1 block break-words text-sm leading-6 text-slate-100">{value}</strong>
    </div>
  );
}

function SettingToggle({ label, checked }: { label: string; checked: boolean }) {
  return (
    <label className="flex min-h-11 cursor-pointer items-center justify-between gap-3 rounded-md border border-white/10 bg-white/[0.03] px-3">
      <span className="text-sm font-bold text-slate-200">{label}</span>
      <input className="sr-only" type="checkbox" defaultChecked={checked} />
      <span className={`relative h-6 w-10 rounded-full ${checked ? "bg-emerald-500" : "bg-slate-700"}`} aria-hidden="true">
        <span className={`absolute top-1 h-4 w-4 rounded-full bg-white transition-transform duration-200 ${checked ? "left-5" : "left-1"}`} />
      </span>
    </label>
  );
}

function SegmentButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      className={`min-h-10 cursor-pointer rounded-md border px-3 text-sm font-black transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-100 ${
        active
          ? "border-emerald-300/60 bg-emerald-500 text-slate-950"
          : "border-white/10 text-slate-300 hover:bg-white/8"
      }`}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}

function MicGlyph() {
  return (
    <svg className="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
      <path d="M5 10v4" />
      <path d="M9 6v12" />
      <path d="M13 4v16" />
      <path d="M17 7v10" />
      <path d="M21 10v4" />
    </svg>
  );
}

function BackGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M19 12H5" />
      <path d="m12 19-7-7 7-7" />
    </svg>
  );
}

function MinimizeGlyph() {
  return (
    <svg className="h-4 w-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
      <path d="M4 8h8" />
    </svg>
  );
}

function MaximizeGlyph() {
  return (
    <svg className="h-4 w-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round">
      <rect x="4" y="4" width="8" height="8" rx="1.2" />
    </svg>
  );
}

function CloseGlyph() {
  return (
    <svg className="h-4 w-4" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
      <path d="M4.5 4.5 11.5 11.5" />
      <path d="m11.5 4.5-7 7" />
    </svg>
  );
}

function PinGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m16 3 5 5" />
      <path d="M19 6 8.5 16.5" />
      <path d="m14 8 2 2" />
      <path d="M5 19 8.5 16.5" />
      <path d="M8 7.5 16.5 16" />
    </svg>
  );
}

function CogGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 1.55V21a2 2 0 0 1-4 0v-.09A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 0 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 0 1 4 0v.09A1.7 1.7 0 0 0 15 4.6a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.55 1H21a2 2 0 0 1 0 4h-.09A1.7 1.7 0 0 0 19.4 15Z" />
    </svg>
  );
}

function InfoGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5" />
      <path d="M12 8h.01" />
    </svg>
  );
}

function FileGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
      <path d="M14 2v6h6" />
      <path d="M8 13h8" />
      <path d="M8 17h5" />
    </svg>
  );
}

function ListGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M8 6h13" />
      <path d="M8 12h13" />
      <path d="M8 18h13" />
      <path d="M3 6h.01" />
      <path d="M3 12h.01" />
      <path d="M3 18h.01" />
    </svg>
  );
}

function ChatGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4Z" />
      <path d="M8 9h8" />
      <path d="M8 13h5" />
    </svg>
  );
}

function EditGlyph() {
  return (
    <svg className="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
    </svg>
  );
}

function SparkGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3 9.5 9.5 3 12l6.5 2.5L12 21l2.5-6.5L21 12l-6.5-2.5Z" />
    </svg>
  );
}

function ThumbGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7 10v10" />
      <path d="M15 5.5 14 10h5.5a2 2 0 0 1 1.95 2.45l-1.15 5A3 3 0 0 1 17.38 20H7" />
      <path d="M7 10H4v10h3" />
    </svg>
  );
}

function ThumbDownGlyph() {
  return (
    <svg className="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7 14V4" />
      <path d="M15 18.5 14 14h5.5a2 2 0 0 0 1.95-2.45l-1.15-5A3 3 0 0 0 17.38 4H7" />
      <path d="M7 14H4V4h3" />
    </svg>
  );
}

function ExternalGlyph() {
  return (
    <svg className="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M15 3h6v6" />
      <path d="M10 14 21 3" />
      <path d="M21 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5" />
    </svg>
  );
}
