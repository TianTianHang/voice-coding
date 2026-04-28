import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBackendVAD } from "../hooks/useBackendVAD";
import { asrStatusLabel, useAsrStatus } from "../hooks/useAsrStatus";
import { useAgentEvents } from "../hooks/useAgentEvents";
import { AgentEventStream } from "./AgentEventStream";
import { AudioVisualizer } from "./AudioVisualizer";
import { ControlButton } from "./ControlButton";

const vadLabels = {
  idle: "Stopped",
  listening: "Listening for the next sentence",
  recording: "Recording current sentence",
  processing: "Transcribing and sending",
};

export function AssistantConsole() {
  const [closeBehavior, setCloseBehaviorState] = useState<"hide" | "exit">(
    "hide",
  );
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

  const speechError = error
    ? error.includes("denied") || error.includes("NotAllowedError")
      ? `${error} Please grant microphone permission in system settings.`
      : error
    : asrStatusError;

  async function setCloseBehavior(behavior: "hide" | "exit") {
    setCloseBehaviorState(behavior);
    try {
      await invoke("set_close_behavior", { behavior });
    } catch {
      setCloseBehaviorState(closeBehavior);
    }
  }

  return (
    <main className="min-h-screen bg-slate-100 p-4 sm:p-5">
      <section className="mx-auto flex min-h-[calc(100vh-2rem)] w-full max-w-3xl flex-col gap-3.5 rounded-lg border border-slate-200 bg-white p-4 shadow-[0_16px_36px_rgba(20,32,36,0.14)] sm:min-h-[calc(100vh-2.5rem)]">
        <header className="flex items-center justify-between gap-3 max-sm:flex-col max-sm:items-stretch">
          <div>
            <h1 className="m-0 text-2xl leading-tight">Voice Agent</h1>
            <p className="mt-1.5 text-sm text-slate-500">{vadLabels[state]}</p>
          </div>
          <div
            className={`max-w-[280px] overflow-hidden rounded-full border px-2.5 py-1.5 text-xs font-bold text-ellipsis whitespace-nowrap max-sm:max-w-full ${
              agent.connectionState === "connected"
                ? "border-emerald-400/40 bg-emerald-600/10 text-emerald-800"
                : agent.connectionState === "error"
                  ? "border-rose-500/30 bg-rose-600/10 text-rose-700"
                  : "border-slate-300 text-slate-500"
            }`}
          >
            {agent.connectionLabel}
          </div>
        </header>

        <section className="grid grid-cols-1 gap-2 sm:grid-cols-3" aria-label="Runtime status">
          <div
            className={`min-h-[62px] rounded-lg border bg-slate-100 p-2.5 ${
              state === "listening"
                ? "border-emerald-400/40"
                : state === "recording"
                  ? "border-rose-500/35"
                  : state === "processing"
                    ? "border-amber-500/35"
                    : "border-slate-300"
            }`}
          >
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">
              Voice
            </span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">
              {vadLabels[state]}
            </strong>
          </div>
          <div
            className={`min-h-[62px] rounded-lg border bg-slate-100 p-2.5 ${
              asrStatus.state === "ready"
                ? "border-emerald-400/40"
                : asrStatus.state === "loading"
                  ? "border-amber-500/35"
                  : asrStatus.state === "failed"
                    ? "border-rose-500/35"
                    : "border-slate-300"
            }`}
          >
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">
              ASR
            </span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">
              {asrStatusLabel(asrStatus)}
            </strong>
          </div>
          <div
            className={`min-h-[62px] rounded-lg border bg-slate-100 p-2.5 ${
              agent.connectionState === "connected"
                ? "border-emerald-400/40"
                : agent.connectionState === "connecting"
                  ? "border-amber-500/35"
                  : agent.connectionState === "error"
                    ? "border-rose-500/35"
                    : "border-slate-300"
            }`}
          >
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">
              Agent
            </span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">
              {agent.connectionLabel}
            </strong>
          </div>
        </section>

        <section className="grid grid-cols-2 gap-2 sm:grid-cols-4" aria-label="Agent session">
          <div className="min-h-[62px] rounded-lg border border-slate-300 bg-slate-100 p-2.5">
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Mode</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.currentModeId ?? "Default"}</strong>
          </div>
          <div className="min-h-[62px] rounded-lg border border-slate-300 bg-slate-100 p-2.5">
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Session</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.sessionInfo.title ?? "Untitled"}</strong>
          </div>
          <div className="min-h-[62px] rounded-lg border border-slate-300 bg-slate-100 p-2.5">
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Commands</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.availableCommands.length}</strong>
          </div>
          <div className="min-h-[62px] rounded-lg border border-slate-300 bg-slate-100 p-2.5">
            <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Config</span>
            <strong className="mt-1 block text-[13px] leading-5 break-words">{agent.sessionState.configOptions.length}</strong>
          </div>
        </section>

        {agent.plan && agent.plan.entries.length > 0 && (
          <section className="rounded-lg border border-slate-300 bg-slate-50 p-3" aria-label="Agent plan">
            <div className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Current plan</div>
            <ol className="mt-2 grid gap-1.5 pl-5">
              {agent.plan.entries.map((entry, index) => (
                <li
                  className={entry.status === "completed" ? "text-slate-500 line-through" : ""}
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

        <section className="flex items-center gap-2.5 max-sm:flex-col max-sm:items-stretch">
          <ControlButton
            state={state}
            onStart={startListening}
            onStop={stopListening}
          />
          <button
            className="min-h-10 rounded-lg bg-slate-100 px-3.5 text-sm font-extrabold text-slate-800 transition-colors hover:bg-slate-200 disabled:cursor-not-allowed disabled:opacity-60"
            onClick={
              agent.connectionState === "connected"
                ? agent.disconnect
                : agent.connect
            }
            disabled={agent.connectionState === "connecting"}
          >
            {agent.connectionState === "connected"
              ? "Disconnect"
              : "Connect Agent"}
          </button>
        </section>

        <section className="flex items-center justify-between gap-3 max-sm:flex-col max-sm:items-stretch" aria-label="Close behavior">
          <span className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Close window</span>
          <div className="grid grid-cols-2 gap-1 rounded-lg border border-slate-300 bg-slate-100 p-1 min-[380px]:min-w-[220px]">
            <button
              className={`min-h-[30px] rounded-md px-2.5 text-xs font-extrabold transition-colors ${
                closeBehavior === "hide"
                  ? "bg-emerald-600 text-white"
                  : "text-slate-500 hover:bg-white"
              }`}
              onClick={() => setCloseBehavior("hide")}
            >
              Hide to tray
            </button>
            <button
              className={`min-h-[30px] rounded-md px-2.5 text-xs font-extrabold transition-colors ${
                closeBehavior === "exit"
                  ? "bg-emerald-600 text-white"
                  : "text-slate-500 hover:bg-white"
              }`}
              onClick={() => setCloseBehavior("exit")}
            >
              Exit app
            </button>
          </div>
        </section>

        <AudioVisualizer state={state} recordingDuration={recordingDuration} />

        <section className="rounded-lg border border-slate-300 bg-slate-50 p-3" aria-label="Current sentence">
          <div className="block text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Current sentence</div>
          <div className="mt-2 min-h-16 whitespace-pre-wrap leading-[1.45]">
            {speechError ? (
              <span className="font-bold text-rose-700">{speechError}</span>
            ) : transcript ? (
              transcript
            ) : (
              <span className="text-slate-500">
                Waiting for your next sentence.
              </span>
            )}
          </div>
        </section>

        <AgentEventStream
          events={agent.events}
          onConfirm={agent.respondToConfirmation}
        />
      </section>
    </main>
  );
}
