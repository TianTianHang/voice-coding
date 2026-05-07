import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  autoTtsStatusLabel,
  type AutoTtsStatusSnapshot,
  type TtsStatusSnapshot,
} from "./AssistantConsole";

type VadRuntimeConfig = {
  threshold: number;
};

type MossSamplingMode = "fixed" | "greedy";

export type TtsInvokeConfig = {
  moss?: {
    samplingMode?: MossSamplingMode;
    referenceAudioPath?: string;
  };
};

export function buildTtsInvokeConfig(
  samplingMode: MossSamplingMode,
  referenceAudioPath: string,
): TtsInvokeConfig {
  const path = referenceAudioPath.trim();
  return {
    moss: {
      samplingMode,
      ...(path ? { referenceAudioPath: path } : {}),
    },
  };
}

export function DebugToolsWindow() {
  const [vadThresholdInput, setVadThresholdInput] = useState("0.5");
  const [vadConfigMessage, setVadConfigMessage] = useState<string | null>(null);
  const [isSavingVadConfig, setIsSavingVadConfig] = useState(false);
  const [ttsText, setTtsText] = useState("你好。");
  const [ttsSamplingMode, setTtsSamplingMode] =
    useState<MossSamplingMode>("fixed");
  const [ttsReferenceAudioPath, setTtsReferenceAudioPath] = useState("");
  const [ttsStatus, setTtsStatus] = useState<TtsStatusSnapshot | null>(null);
  const [autoTtsStatus, setAutoTtsStatus] =
    useState<AutoTtsStatusSnapshot | null>(null);
  const [ttsMessage, setTtsMessage] = useState<string | null>(null);
  const [isSynthesizingTts, setIsSynthesizingTts] = useState(false);
  const [isPlayingTts, setIsPlayingTts] = useState(false);
  const [isUpdatingAutoTts, setIsUpdatingAutoTts] = useState(false);

  useEffect(() => {
    let active = true;

    async function loadRuntimeState() {
      try {
        const config = await invoke<VadRuntimeConfig>("get_vad_config");
        if (active) {
          setVadThresholdInput(config.threshold.toString());
        }
      } catch {
        if (active) {
          setVadConfigMessage("Failed to load VAD threshold.");
        }
      }

      try {
        const status = await invoke<TtsStatusSnapshot>("get_tts_status");
        if (active) {
          setTtsStatus(status);
        }
      } catch {
        if (active) {
          setTtsMessage("Failed to load TTS status.");
        }
      }

      try {
        const status = await invoke<AutoTtsStatusSnapshot>("get_auto_tts_status");
        if (active) {
          setAutoTtsStatus(status);
          setTtsStatus(status.tts);
        }
      } catch {
        if (active) {
          setTtsMessage("Failed to load auto speech status.");
        }
      }
    }

    void loadRuntimeState();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    async function setupAutoTtsEvents() {
      unlisten = await listen<AutoTtsStatusSnapshot>("auto-tts-state", (event) => {
        setAutoTtsStatus(event.payload);
        setTtsStatus(event.payload.tts);
      });
    }

    void setupAutoTtsEvents();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  async function applyVadThreshold() {
    const parsed = Number(vadThresholdInput);
    if (!Number.isFinite(parsed)) {
      setVadConfigMessage("Threshold must be a number between 0.0 and 1.0.");
      return;
    }

    setIsSavingVadConfig(true);
    setVadConfigMessage(null);
    try {
      await invoke("set_vad_config", {
        config: { threshold: parsed },
      });
      setVadConfigMessage(
        `Saved threshold ${parsed.toFixed(3)}. Takes effect next Start.`,
      );
    } catch (error) {
      setVadConfigMessage(String(error));
    } finally {
      setIsSavingVadConfig(false);
    }
  }

  async function synthesizeTts() {
    const text = ttsText.trim();
    if (!text) {
      setTtsMessage("TTS text must not be empty.");
      return;
    }

    setIsSynthesizingTts(true);
    setTtsMessage(null);
    try {
      const status = await invoke<TtsStatusSnapshot>("synthesize_tts", {
        text,
        config: buildTtsInvokeConfig(ttsSamplingMode, ttsReferenceAudioPath),
      });
      setTtsStatus(status);
      setTtsMessage(
        status.hasBufferedAudio
          ? "Audio generated and ready to play."
          : "Synthesis finished without buffered audio.",
      );
    } catch (error) {
      setTtsMessage(String(error));
      setTtsStatus((current) =>
        current ? { ...current, state: "failed", error: String(error) } : null,
      );
    } finally {
      setIsSynthesizingTts(false);
    }
  }

  async function playTts() {
    setIsPlayingTts(true);
    setTtsMessage(null);
    try {
      const status = await invoke<TtsStatusSnapshot>("play_tts");
      setTtsStatus(status);
      setTtsMessage("Playback finished.");
    } catch (error) {
      setTtsMessage(String(error));
      setTtsStatus((current) =>
        current ? { ...current, state: "failed", error: String(error) } : null,
      );
    } finally {
      setIsPlayingTts(false);
    }
  }

  async function cancelTtsPlayback() {
    setTtsMessage(null);
    try {
      const status = await invoke<TtsStatusSnapshot>("cancel_tts_playback");
      setTtsStatus(status);
      setTtsMessage("Playback cancelled.");
    } catch (error) {
      setTtsMessage(String(error));
    }
  }

  async function setAutoTtsEnabled(enabled: boolean) {
    setIsUpdatingAutoTts(true);
    setTtsMessage(null);
    try {
      const status = await invoke<AutoTtsStatusSnapshot>(
        "set_auto_tts_enabled",
        { enabled },
      );
      setAutoTtsStatus(status);
      setTtsStatus(status.tts);
    } catch (error) {
      setTtsMessage(String(error));
    } finally {
      setIsUpdatingAutoTts(false);
    }
  }

  async function stopAutoTts() {
    setIsUpdatingAutoTts(true);
    setTtsMessage(null);
    try {
      const status = await invoke<AutoTtsStatusSnapshot>("stop_auto_tts");
      setAutoTtsStatus(status);
      setTtsStatus(status.tts);
    } catch (error) {
      setTtsMessage(String(error));
    } finally {
      setIsUpdatingAutoTts(false);
    }
  }

  async function speakLatestResult() {
    setIsUpdatingAutoTts(true);
    setTtsMessage(null);
    try {
      const status = await invoke<AutoTtsStatusSnapshot>("speak_latest_result");
      setAutoTtsStatus(status);
      setTtsStatus(status.tts);
    } catch (error) {
      setTtsMessage(String(error));
    } finally {
      setIsUpdatingAutoTts(false);
    }
  }

  return (
    <main className="voice-shell min-h-screen p-3 text-slate-950 sm:p-4">
      <section className="debug-window mx-auto flex min-h-[calc(100vh-1.5rem)] w-full max-w-2xl flex-col gap-4 rounded-lg border border-slate-200 bg-white p-4 shadow-[0_18px_48px_rgba(15,23,42,0.10)] sm:min-h-[calc(100vh-2rem)]">
        <header className="border-b border-slate-200 pb-3">
          <h1 className="m-0 text-xl font-black leading-tight text-slate-950">
            Debug Tools
          </h1>
          <p className="mt-1 text-sm text-slate-600">
            VAD threshold and TTS runtime controls
          </p>
        </header>

        <section className="debug-panel" aria-label="Developer VAD controls">
          <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
            VAD Threshold
          </div>
          <div className="grid gap-2">
            <div className="flex items-center gap-2">
              <input
                className="min-h-10 w-full rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="number"
                min={0}
                max={1}
                step={0.01}
                value={vadThresholdInput}
                onChange={(event) => setVadThresholdInput(event.target.value)}
                placeholder="0.0 - 1.0"
              />
              <button
                className="min-h-10 shrink-0 cursor-pointer rounded-lg border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={applyVadThreshold}
                disabled={isSavingVadConfig}
                type="button"
              >
                {isSavingVadConfig ? "Saving..." : "Apply"}
              </button>
            </div>
            <p className="text-xs text-slate-600">
              Lower is more sensitive; higher is stricter.
            </p>
            {vadConfigMessage && (
              <p className="text-xs font-semibold text-slate-700">
                {vadConfigMessage}
              </p>
            )}
          </div>
        </section>

        <section className="debug-panel" aria-label="Developer TTS controls">
          <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
            Auto Speech
          </div>
          <div className="mb-4 grid gap-2">
            <div className="grid grid-cols-2 gap-2">
              <button
                className={`min-h-10 cursor-pointer rounded-lg border px-3 text-sm font-extrabold transition-colors duration-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60 ${
                  autoTtsStatus?.enabled
                    ? "border-slate-950 bg-slate-950 text-white"
                    : "border-slate-300 bg-white text-slate-800 hover:bg-slate-50"
                }`}
                onClick={() => setAutoTtsEnabled(!autoTtsStatus?.enabled)}
                disabled={isUpdatingAutoTts}
                type="button"
              >
                {autoTtsStatus?.enabled ? "Auto On" : "Auto Off"}
              </button>
              <button
                className="min-h-10 cursor-pointer rounded-lg border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={speakLatestResult}
                disabled={isUpdatingAutoTts || !autoTtsStatus?.latestResultText}
                type="button"
              >
                Replay
              </button>
            </div>
            <button
              className="min-h-10 cursor-pointer rounded-lg border border-rose-300 bg-rose-50 px-3 text-sm font-extrabold text-rose-800 transition-colors duration-200 hover:bg-rose-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-900 disabled:cursor-not-allowed disabled:opacity-60"
              onClick={stopAutoTts}
              disabled={isUpdatingAutoTts || !autoTtsStatus?.isPlaying}
              type="button"
            >
              Stop Auto Speech
            </button>
            <p className="text-xs text-slate-600">
              {autoTtsStatusLabel(autoTtsStatus)}
            </p>
          </div>

          <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
            TTS Test
          </div>
          <div className="grid gap-2">
            <textarea
              className="min-h-24 resize-y rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
              value={ttsText}
              onChange={(event) => setTtsText(event.target.value)}
              placeholder="Text to synthesize"
            />
            <div className="grid grid-cols-[minmax(0,0.75fr)_minmax(0,1.25fr)] gap-2">
              <select
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                value={ttsSamplingMode}
                onChange={(event) =>
                  setTtsSamplingMode(event.target.value as MossSamplingMode)
                }
                aria-label="MOSS sampling mode"
              >
                <option value="fixed">fixed</option>
                <option value="greedy">greedy</option>
              </select>
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="text"
                value={ttsReferenceAudioPath}
                onChange={(event) =>
                  setTtsReferenceAudioPath(event.target.value)
                }
                placeholder="Reference WAV path"
                aria-label="Reference audio path"
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <button
                className="min-h-10 cursor-pointer rounded-lg border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={synthesizeTts}
                disabled={isSynthesizingTts || isPlayingTts}
                type="button"
              >
                {isSynthesizingTts ? "Generating..." : "Generate"}
              </button>
              <button
                className="min-h-10 cursor-pointer rounded-lg border border-slate-300 bg-white px-3 text-sm font-extrabold text-slate-800 transition-colors duration-200 hover:bg-slate-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={playTts}
                disabled={
                  isSynthesizingTts || isPlayingTts || !ttsStatus?.hasBufferedAudio
                }
                type="button"
              >
                {isPlayingTts ? "Playing..." : "Play"}
              </button>
            </div>
            {isPlayingTts && (
              <button
                className="min-h-10 cursor-pointer rounded-lg border border-rose-300 bg-rose-50 px-3 text-sm font-extrabold text-rose-800 transition-colors duration-200 hover:bg-rose-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-rose-900"
                onClick={cancelTtsPlayback}
                type="button"
              >
                Stop Playback
              </button>
            )}
            <p className="text-xs text-slate-600">
              Status:{" "}
              {ttsStatus
                ? `${ttsStatus.state}${ttsStatus.hasBufferedAudio ? " · buffered" : ""}`
                : "unknown"}
            </p>
            {ttsStatus && (
              <p className="text-xs text-slate-600">
                Engine: {ttsStatus.engineName} · Model:{" "}
                {ttsStatus.model.modelDir || "unresolved"}
              </p>
            )}
            {(ttsMessage || ttsStatus?.error) && (
              <p className="text-xs font-semibold text-slate-700">
                {ttsMessage || ttsStatus?.error}
              </p>
            )}
          </div>
        </section>
      </section>
    </main>
  );
}
