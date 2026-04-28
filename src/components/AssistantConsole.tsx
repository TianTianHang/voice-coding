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
    <main className="assistant-shell">
      <section className="assistant-panel">
        <header className="panel-header">
          <div>
            <h1>Voice Agent</h1>
            <p>{vadLabels[state]}</p>
          </div>
          <div className={`connection-pill connection-${agent.connectionState}`}>
            {agent.connectionLabel}
          </div>
        </header>

        <section className="status-grid" aria-label="Runtime status">
          <div className={`status-item status-${state}`}>
            <span>Voice</span>
            <strong>{vadLabels[state]}</strong>
          </div>
          <div className={`status-item status-asr-${asrStatus.state}`}>
            <span>ASR</span>
            <strong>{asrStatusLabel(asrStatus)}</strong>
          </div>
          <div className={`status-item status-agent-${agent.connectionState}`}>
            <span>Agent</span>
            <strong>{agent.connectionLabel}</strong>
          </div>
        </section>

        <section className="agent-session-summary" aria-label="Agent session">
          <div className="session-summary-item">
            <span className="section-label">Mode</span>
            <strong>{agent.sessionState.currentModeId ?? "Default"}</strong>
          </div>
          <div className="session-summary-item">
            <span className="section-label">Session</span>
            <strong>{agent.sessionState.sessionInfo.title ?? "Untitled"}</strong>
          </div>
          <div className="session-summary-item">
            <span className="section-label">Commands</span>
            <strong>{agent.sessionState.availableCommands.length}</strong>
          </div>
          <div className="session-summary-item">
            <span className="section-label">Config</span>
            <strong>{agent.sessionState.configOptions.length}</strong>
          </div>
        </section>

        {agent.plan && agent.plan.entries.length > 0 && (
          <section className="agent-plan" aria-label="Agent plan">
            <div className="section-label">Current plan</div>
            <ol>
              {agent.plan.entries.map((entry, index) => (
                <li className={`plan-entry plan-entry-${entry.status}`} key={index}>
                  <span>{entry.content}</span>
                  <small>
                    {entry.priority} · {entry.status.replace("_", " ")}
                  </small>
                </li>
              ))}
            </ol>
          </section>
        )}

        <section className="control-row">
          <ControlButton
            state={state}
            onStart={startListening}
            onStop={stopListening}
          />
          <button
            className="secondary-button"
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

        <section className="close-behavior" aria-label="Close behavior">
          <span className="section-label">Close window</span>
          <div className="segmented-control">
            <button
              className={closeBehavior === "hide" ? "active" : ""}
              onClick={() => setCloseBehavior("hide")}
            >
              Hide to tray
            </button>
            <button
              className={closeBehavior === "exit" ? "active" : ""}
              onClick={() => setCloseBehavior("exit")}
            >
              Exit app
            </button>
          </div>
        </section>

        <AudioVisualizer state={state} recordingDuration={recordingDuration} />

        <section className="current-utterance" aria-label="Current sentence">
          <div className="section-label">Current sentence</div>
          <div className="utterance-text">
            {speechError ? (
              <span className="utterance-error">{speechError}</span>
            ) : transcript ? (
              transcript
            ) : (
              <span className="utterance-placeholder">
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
