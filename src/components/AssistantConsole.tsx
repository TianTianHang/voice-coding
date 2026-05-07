import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
import { AgentEventStream } from "./AgentEventStream";
import { AudioVisualizer } from "./AudioVisualizer";
import { ControlButton } from "./ControlButton";

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
    headline: "Ready",
    detail: "Say the wake phrase when you want to code.",
    status: "Waiting for voice activation",
    intent: "No active request",
  },
  WakeDetected: {
    headline: "Awake",
    detail: "Wake phrase heard. Keep speaking.",
    status: "Activation confirmed",
    intent: "Preparing to listen",
  },
  Listening: {
    headline: "Listening",
    detail: "Capturing your current sentence.",
    status: "Collecting speech",
    intent: "Understanding your request",
  },
  Processing: {
    headline: "Working",
    detail: "Transcribing, sending, or waiting on the agent.",
    status: "Processing the request",
    intent: "Routing work to the agent",
  },
  Responding: {
    headline: "Responding",
    detail: "Showing the agent's latest answer.",
    status: "Response available",
    intent: "Answering the current turn",
  },
  Error: {
    headline: "Error",
    detail: "A recovery step is needed before voice flow can continue.",
    status: "Needs attention",
    intent: "Recover the session",
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
    return "Auto speech unknown";
  }
  if (!status.enabled) {
    return "Auto speech off";
  }
  if (status.isPlaying || status.lastStatus === "speaking") {
    return "Speaking reply";
  }
  if (status.lastStatus === "skippedDuplicate") {
    return "Duplicate skipped";
  }
  if (
    status.lastStatus === "skippedMissingTag" ||
    status.lastStatus === "skippedInvalidTag" ||
    status.lastStatus === "skippedEmptyTag"
  ) {
    return "Auto speech skipped";
  }
  if (status.lastStatus === "failed") {
    return status.tts.error ? `Auto speech failed: ${status.tts.error}` : "Auto speech failed";
  }
  return "Auto speech on";
}

export function AssistantConsole() {
  const [closeBehavior, setCloseBehaviorState] = useState<"hide" | "exit">(
    "hide",
  );
  const [wakeDetected, setWakeDetected] = useState(false);
  const [timelineExpanded, setTimelineExpanded] = useState(false);
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
  const latestAgentEvent =
    agent.events.length > 0 ? agent.events[agent.events.length - 1] : undefined;

  useEffect(() => {
    let active = true;

    async function loadAutoTtsStatus() {
      try {
        const status = await invoke<AutoTtsStatusSnapshot>("get_auto_tts_status");
        if (!active) {
          return;
        }
        setAutoTtsStatus(status);
      } catch {
        if (!active) {
          return;
        }
      }
    }

    void loadAutoTtsStatus();

    return () => {
      active = false;
    };
  }, []);

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
    if (shouldExpandTimelineForEvent(latestAgentEvent)) {
      setTimelineExpanded(true);
    }
  }, [latestAgentEvent?.id, latestAgentEvent?.kind]);

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
      heard: speechError || transcript || "Waiting for your next sentence.",
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
      response: responseEvent?.content || "No response yet.",
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

  return (
    <main className="voice-shell min-h-screen p-3 text-slate-950 sm:p-5">
      <section className="voice-console mx-auto flex min-h-[calc(100vh-1.5rem)] w-full max-w-6xl flex-col gap-4 rounded-lg border border-slate-200 bg-white p-4 shadow-[0_18px_48px_rgba(15,23,42,0.10)] sm:min-h-[calc(100vh-2.5rem)] sm:p-5">
        <header className="flex items-center justify-between gap-3 border-b border-slate-200 pb-3 max-sm:flex-col max-sm:items-stretch">
          <div>
            <h1 className="m-0 text-xl font-black leading-tight text-slate-950">Voice Agent</h1>
            <p className="mt-1 text-sm text-slate-600">
              语音优先工作台 · {copy.headline}
            </p>
          </div>
          <div
            className={`max-w-[320px] overflow-hidden rounded-md border px-2.5 py-1.5 text-xs font-bold text-ellipsis whitespace-nowrap max-sm:max-w-full ${
              agent.connectionState === "connected"
                ? "border-emerald-500/40 bg-emerald-50 text-emerald-800"
                : agent.connectionState === "error"
                  ? "border-rose-500/35 bg-rose-50 text-rose-700"
                  : "border-slate-300 bg-slate-50 text-slate-600"
            }`}
          >
            {agent.connectionLabel}
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
                  <div className="presence-orb-core" />
                </div>
              </div>
              <div className="min-w-0">
                <div className="text-[11px] font-extrabold uppercase text-slate-500">
                  {experienceState}
                </div>
                <h2 className="mt-1 text-4xl font-black leading-none text-slate-950 max-sm:text-3xl">
                  {copy.headline}
                </h2>
              </div>
            </div>
            <p className="mt-4 max-w-[42rem] text-base leading-7 text-slate-600">
              {copy.detail}
            </p>
            <div className="mt-5 rounded-lg border border-slate-200 bg-slate-50 p-3">
              <AudioVisualizer state={state} recordingDuration={recordingDuration} />
            </div>
          </section>

          <aside className="side-panel" aria-label="Session and fallback controls">
            <section className="grid gap-2" aria-label="Runtime status">
              <StatusRow label="Voice" value={copy.status} tone={experienceState} />
              <StatusRow label="ASR" value={asrStatusLabel(asrStatus)} />
              <StatusRow label="Agent" value={agent.connectionLabel} tone={agent.connectionState === "error" ? "Error" : agent.connectionState === "connected" ? "Listening" : "Dormant"} />
              <StatusRow label="Speech" value={autoTtsStatusLabel(autoTtsStatus)} tone={autoTtsStatus?.isPlaying ? "Responding" : "Dormant"} />
            </section>

            <section className="mt-4 border-t border-slate-200 pt-4" aria-label="Fallback controls">
              <div className="text-[11px] font-extrabold uppercase text-slate-500">
                Fallback
              </div>
              <div className="mt-2 grid gap-2">
                <ControlButton
                  state={state}
                  onStart={startListening}
                  onStop={stopListening}
                />
                <button
                  className="min-h-10 cursor-pointer rounded-lg border border-slate-300 bg-white px-3.5 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                  onClick={
                    agent.connectionState === "connected"
                      ? agent.disconnect
                      : agent.connect
                  }
                  disabled={agent.connectionState === "connecting"}
                  type="button"
                >
                  {agent.connectionState === "connected"
                    ? "Disconnect Agent"
                    : "Connect Agent"}
                </button>
              </div>
            </section>

            <section className="mt-4 border-t border-slate-200 pt-4" aria-label="Close behavior">
              <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
                Close window
              </div>
              <div className="grid grid-cols-2 gap-1 rounded-lg border border-slate-300 bg-slate-100 p-1">
                <button
                  className={`min-h-[30px] cursor-pointer rounded-md px-2.5 text-xs font-extrabold transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 ${
                    closeBehavior === "hide"
                      ? "bg-slate-950 text-white"
                      : "text-slate-600 hover:bg-white"
                  }`}
                  onClick={() => setCloseBehavior("hide")}
                  type="button"
                >
                  Hide
                </button>
                <button
                  className={`min-h-[30px] cursor-pointer rounded-md px-2.5 text-xs font-extrabold transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 ${
                    closeBehavior === "exit"
                      ? "bg-slate-950 text-white"
                      : "text-slate-600 hover:bg-white"
                  }`}
                  onClick={() => setCloseBehavior("exit")}
                  type="button"
                >
                  Exit
                </button>
              </div>
            </section>

            <section className="mt-4 border-t border-slate-200 pt-4" aria-label="Developer tools">
              <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
                Debug
              </div>
              <div className="grid gap-2">
                <button
                  className="min-h-10 cursor-pointer rounded-lg border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                  onClick={openDebugToolsWindow}
                  type="button"
                >
                  Open Debug Tools
                </button>
                {debugWindowMessage && (
                  <p className="text-xs font-semibold text-slate-700">{debugWindowMessage}</p>
                )}
              </div>
            </section>
          </aside>
        </section>

        <section className="feedback-layout" aria-label="Voice feedback loop">
          <FeedbackTile label="Heard" value={feedback.heard} urgent={Boolean(speechError)} />
          <FeedbackTile label="Intent" value={feedback.intent} />
          <FeedbackTile label="Status" value={feedback.status} urgent={experienceState === "Error"} />
          <FeedbackTile label="Response" value={feedback.response} featured />
        </section>

        {agent.plan && agent.plan.entries.length > 0 && (
          <section className="rounded-lg border border-slate-200 bg-white p-3" aria-label="Agent plan">
            <div className="block text-[11px] font-extrabold uppercase text-slate-500">Current plan</div>
            <ol className="mt-2 grid gap-1.5 pl-5 text-sm">
              {agent.plan.entries.map((entry, index) => (
                <li
                  className={entry.status === "completed" ? "text-slate-500 line-through" : "text-slate-800"}
                  key={index}
                >
                  <span className="block">{entry.content}</span>
                  <small className="mt-0.5 block text-[11px] font-extrabold capitalize text-slate-500">
                    {entry.priority} · {entry.status.replace("_", " ")}
                  </small>
                </li>
              ))}
            </ol>
          </section>
        )}

        <AgentEventStream
          events={agent.events}
          expanded={timelineExpanded}
          onConfirm={agent.respondToConfirmation}
          onToggleExpanded={() => setTimelineExpanded((expanded) => !expanded)}
        />

        <section className="session-strip" aria-label="Agent session">
          <div>
            <span className="block text-[11px] font-extrabold uppercase text-slate-500">Mode</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.currentModeId ?? "Default"}</strong>
          </div>
          <div>
            <span className="block text-[11px] font-extrabold uppercase text-slate-500">Session</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.sessionInfo.title ?? "Untitled"}</strong>
          </div>
          <div>
            <span className="block text-[11px] font-extrabold uppercase text-slate-500">Commands</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.availableCommands.length}</strong>
          </div>
          <div>
            <span className="block text-[11px] font-extrabold uppercase text-slate-500">Config</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.configOptions.length}</strong>
          </div>
        </section>
      </section>
    </main>
  );
}

function FeedbackTile({
  label,
  value,
  urgent = false,
  featured = false,
}: {
  label: string;
  value: string;
  urgent?: boolean;
  featured?: boolean;
}) {
  return (
    <div
      className={`feedback-tile min-h-[112px] rounded-lg border p-3 ${featured ? "feedback-tile-featured" : ""} ${
        urgent
          ? "border-rose-500/35 bg-rose-50 text-rose-900"
          : "border-slate-200 bg-white text-slate-900"
      }`}
    >
      <span className="block text-[11px] font-extrabold uppercase text-slate-500">
        {label}
      </span>
      <strong className="mt-2 block max-h-24 overflow-hidden text-sm leading-5">
        {value}
      </strong>
    </div>
  );
}

function StatusRow({
  label,
  value,
  tone = "Dormant",
}: {
  label: string;
  value: string;
  tone?: VoiceExperienceState;
}) {
  return (
    <div className="status-row">
      <span className={`status-dot status-dot-${tone.toLowerCase()}`} />
      <div className="min-w-0">
        <span className="block text-[11px] font-extrabold uppercase text-slate-500">
          {label}
        </span>
        <strong className="block truncate text-sm text-slate-900">{value}</strong>
      </div>
    </div>
  );
}
