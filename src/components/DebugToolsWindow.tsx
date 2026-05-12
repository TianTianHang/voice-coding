import { useEffect, useRef, useState } from "react";
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

type AsrDebugSourceKind = "url" | "file";

type DebugStreamingAsrEvent = {
  runId: string;
  kind: "started" | "partial" | "final" | "end";
  text: string;
  language?: string | null;
  endTimeSec?: number | null;
};

type DebugStreamingAsrResult = {
  runId: string;
  text: string;
  language: string;
  audioDurationSec: number;
  processingTimeSec: number;
  rtf: number;
  tokensGenerated?: number | null;
  events: DebugStreamingAsrEvent[];
};

export type DebugStreamingAsrInvokeRequest = {
  runId: string;
  sourceKind: AsrDebugSourceKind;
  source: string;
  audioData?: number[];
  language?: string;
  chunkSeconds?: number;
  unfixedChunkNum?: number;
  unfixedTokenNum?: number;
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

export function buildDebugStreamingAsrRequest(
  runId: string,
  sourceKind: AsrDebugSourceKind,
  source: string,
  audioData: number[] | undefined,
  language: string,
  chunkSeconds: string,
  unfixedChunkNum: string,
  unfixedTokenNum: string,
): DebugStreamingAsrInvokeRequest {
  const request: DebugStreamingAsrInvokeRequest = {
    runId,
    sourceKind,
    source: source.trim(),
  };
  if (audioData && audioData.length > 0) {
    request.audioData = audioData;
  }
  const trimmedLanguage = language.trim();
  if (trimmedLanguage) {
    request.language = trimmedLanguage;
  }

  const parsedChunkSeconds = optionalFiniteNumber(chunkSeconds);
  if (parsedChunkSeconds !== undefined) {
    request.chunkSeconds = parsedChunkSeconds;
  }

  const parsedUnfixedChunkNum = optionalNonNegativeInteger(unfixedChunkNum);
  if (parsedUnfixedChunkNum !== undefined) {
    request.unfixedChunkNum = parsedUnfixedChunkNum;
  }

  const parsedUnfixedTokenNum = optionalNonNegativeInteger(unfixedTokenNum);
  if (parsedUnfixedTokenNum !== undefined) {
    request.unfixedTokenNum = parsedUnfixedTokenNum;
  }

  return request;
}

export function createDebugAsrRunId(now = Date.now()): string {
  return `asr-debug-${now}-${Math.random().toString(36).slice(2, 8)}`;
}

export function formatAsrDebugTime(seconds?: number | null): string {
  if (seconds === undefined || seconds === null || !Number.isFinite(seconds)) {
    return "--";
  }
  return `${seconds.toFixed(2)}s`;
}

function optionalFiniteNumber(value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function optionalNonNegativeInteger(value: string): number | undefined {
  const parsed = optionalFiniteNumber(value);
  if (parsed === undefined) {
    return undefined;
  }
  return Math.max(0, Math.floor(parsed));
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
  const [asrSourceKind, setAsrSourceKind] =
    useState<AsrDebugSourceKind>("file");
  const [asrSource, setAsrSource] = useState("");
  const [asrSelectedFileName, setAsrSelectedFileName] = useState("");
  const [asrSelectedFileData, setAsrSelectedFileData] = useState<number[]>();
  const [asrLanguage, setAsrLanguage] = useState("");
  const [asrChunkSeconds, setAsrChunkSeconds] = useState("2");
  const [asrUnfixedChunkNum, setAsrUnfixedChunkNum] = useState("2");
  const [asrUnfixedTokenNum, setAsrUnfixedTokenNum] = useState("5");
  const [asrResult, setAsrResult] = useState<DebugStreamingAsrResult | null>(
    null,
  );
  const [asrEvents, setAsrEvents] = useState<DebugStreamingAsrEvent[]>([]);
  const activeAsrRunIdRef = useRef<string | null>(null);
  const [showAsrOutput, setShowAsrOutput] = useState(false);
  const [liveAsrText, setLiveAsrText] = useState("");
  const [targetLiveAsrText, setTargetLiveAsrText] = useState("");
  const [asrMessage, setAsrMessage] = useState<string | null>(null);
  const [isTestingAsr, setIsTestingAsr] = useState(false);

  useEffect(() => {
    let active = true;

    async function loadRuntimeState() {
      try {
        const config = await invoke<VadRuntimeConfig>("debug_get_vad_config");
        if (active) {
          setVadThresholdInput(config.threshold.toString());
        }
      } catch {
        if (active) {
          setVadConfigMessage("Failed to load VAD threshold.");
        }
      }

      try {
        const status = await invoke<TtsStatusSnapshot>("debug_get_tts_status");
        if (active) {
          setTtsStatus(status);
        }
      } catch {
        if (active) {
          setTtsMessage("Failed to load TTS status.");
        }
      }

      try {
        const status = await invoke<AutoTtsStatusSnapshot>("debug_get_auto_tts_status");
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

    async function setupDebugAsrEvents() {
      unlisten = await listen<DebugStreamingAsrEvent>(
        "debug-streaming-asr",
        (event) => {
          const payload = event.payload;
          if (activeAsrRunIdRef.current !== payload.runId) {
            return;
          }

          setAsrEvents((current) => [...current, payload]);
          if (payload.kind === "started") {
            setAsrMessage("Streaming ASR started.");
            setLiveAsrText("");
            setTargetLiveAsrText("");
          } else {
            setTargetLiveAsrText(payload.text);
            setAsrMessage(
              payload.kind === "end"
                ? "Streaming ASR finished."
                : `Streaming ASR updated at ${formatAsrDebugTime(payload.endTimeSec)}.`,
            );
          }
        },
      );
    }

    void setupDebugAsrEvents();

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (liveAsrText === targetLiveAsrText) {
      return;
    }

    const timer = window.setTimeout(() => {
      setLiveAsrText((current) => {
        if (current === targetLiveAsrText) {
          return current;
        }

        if (!targetLiveAsrText.startsWith(current)) {
          return targetLiveAsrText.slice(0, 1);
        }

        const nextLength = Math.min(current.length + 1, targetLiveAsrText.length);
        return targetLiveAsrText.slice(0, nextLength);
      });
    }, 18);

    return () => window.clearTimeout(timer);
  }, [liveAsrText, targetLiveAsrText]);

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
      await invoke("debug_set_vad_config", {
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
      const status = await invoke<TtsStatusSnapshot>("debug_synthesize_tts", {
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
      const status = await invoke<TtsStatusSnapshot>("debug_play_tts");
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
      const status = await invoke<TtsStatusSnapshot>("debug_cancel_tts_playback");
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
        "debug_set_auto_tts_enabled",
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
      const status = await invoke<AutoTtsStatusSnapshot>("debug_stop_auto_tts");
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
      const status = await invoke<AutoTtsStatusSnapshot>("debug_speak_latest_result");
      setAutoTtsStatus(status);
      setTtsStatus(status.tts);
    } catch (error) {
      setTtsMessage(String(error));
    } finally {
      setIsUpdatingAutoTts(false);
    }
  }

  async function testStreamingAsr() {
    const source = asrSource.trim();
    if (!source && !asrSelectedFileData?.length) {
      setAsrMessage("ASR source must not be empty.");
      return;
    }

    setIsTestingAsr(true);
    const runId = createDebugAsrRunId();
    activeAsrRunIdRef.current = runId;
    setAsrMessage(null);
    setAsrResult(null);
    setAsrEvents([]);
    setShowAsrOutput(true);
    setLiveAsrText("");
    setTargetLiveAsrText("");
    try {
      const result = await invoke<DebugStreamingAsrResult>(
        "debug_streaming_asr",
        {
          request: buildDebugStreamingAsrRequest(
            runId,
            asrSourceKind,
            source,
            asrSourceKind === "file" ? asrSelectedFileData : undefined,
            asrLanguage,
            asrChunkSeconds,
            asrUnfixedChunkNum,
            asrUnfixedTokenNum,
          ),
        },
      );
      if (result.runId === runId) {
        setAsrResult(result);
        setTargetLiveAsrText(result.text);
        setAsrMessage(
          `Streaming ASR finished with ${result.events.length} events.`,
        );
      }
    } catch (error) {
      setAsrMessage(String(error));
    } finally {
      setIsTestingAsr(false);
    }
  }

  async function selectAsrFile(file: File | null) {
    if (!file) {
      setAsrSelectedFileName("");
      setAsrSelectedFileData(undefined);
      return;
    }

    setAsrSelectedFileName(file.name);
    setAsrMessage(null);
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      setAsrSelectedFileData(Array.from(data));
      setAsrSource(file.name);
    } catch (error) {
      setAsrSelectedFileData(undefined);
      setAsrMessage(String(error));
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
            VAD threshold, streaming ASR, and TTS runtime controls
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

        <section className="debug-panel" aria-label="Developer streaming ASR controls">
          <div className="mb-2 text-[11px] font-extrabold uppercase text-slate-500">
            Streaming ASR Test
          </div>
          <div className="grid gap-2">
            <div className="grid grid-cols-[minmax(0,0.7fr)_minmax(0,1.3fr)] gap-2">
              <select
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                value={asrSourceKind}
                onChange={(event) =>
                  setAsrSourceKind(event.target.value as AsrDebugSourceKind)
                }
                aria-label="Streaming ASR source type"
              >
                <option value="file">file path</option>
                <option value="url">URL</option>
              </select>
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="text"
                value={asrSource}
                onChange={(event) => setAsrSource(event.target.value)}
                placeholder={
                  asrSourceKind === "url"
                    ? "https://example.com/audio.wav"
                    : "/absolute/path/to/audio.wav or choose file"
                }
                aria-label="Streaming ASR source"
              />
            </div>
            {asrSourceKind === "file" && (
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-900 file:mr-3 file:cursor-pointer file:rounded-md file:border-0 file:bg-slate-950 file:px-3 file:py-1.5 file:text-xs file:font-extrabold file:text-white focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="file"
                accept="audio/*,.wav,.mp3,.m4a,.flac,.ogg"
                onChange={(event) =>
                  void selectAsrFile(event.currentTarget.files?.[0] ?? null)
                }
                aria-label="Streaming ASR audio file"
              />
            )}
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="text"
                value={asrLanguage}
                onChange={(event) => setAsrLanguage(event.target.value)}
                placeholder="lang"
                aria-label="Streaming ASR language"
              />
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="number"
                min={0.1}
                step={0.1}
                value={asrChunkSeconds}
                onChange={(event) => setAsrChunkSeconds(event.target.value)}
                aria-label="Streaming ASR chunk seconds"
              />
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="number"
                min={0}
                step={1}
                value={asrUnfixedChunkNum}
                onChange={(event) => setAsrUnfixedChunkNum(event.target.value)}
                aria-label="Streaming ASR unfixed chunk count"
              />
              <input
                className="min-h-10 rounded-lg border border-slate-300 bg-white px-3 text-sm font-semibold text-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900"
                type="number"
                min={0}
                step={1}
                value={asrUnfixedTokenNum}
                onChange={(event) => setAsrUnfixedTokenNum(event.target.value)}
                aria-label="Streaming ASR unfixed token count"
              />
            </div>
            <button
              className="min-h-10 cursor-pointer rounded-lg border border-slate-950 bg-slate-950 px-3 text-sm font-extrabold text-white transition-colors duration-200 hover:bg-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 disabled:cursor-not-allowed disabled:opacity-60"
              onClick={testStreamingAsr}
              disabled={isTestingAsr}
              type="button"
            >
              {isTestingAsr ? "Running..." : "Run Streaming ASR"}
            </button>
            <p className="text-xs text-slate-600">
              Fields: language, chunk seconds, unfixed chunks, unfixed tokens.
              {asrSelectedFileName ? ` Selected: ${asrSelectedFileName}` : ""}
            </p>
            {asrMessage && (
              <p className="text-xs font-semibold text-slate-700">
                {asrMessage}
              </p>
            )}
            {showAsrOutput && (
              <div className="grid gap-2">
                <div className="rounded-lg border border-slate-200 bg-white p-3">
                  <div className="mb-1 text-[11px] font-extrabold uppercase text-slate-500">
                    {isTestingAsr ? "Live Transcript" : "Final Transcript"}
                  </div>
                  <p className="whitespace-pre-wrap text-sm font-semibold text-slate-900">
                    {(liveAsrText || asrResult?.text) || "(empty)"}
                  </p>
                  {isTestingAsr && !liveAsrText && (
                    <p className="mt-2 text-xs font-semibold text-slate-500">
                      Waiting for first streaming update...
                    </p>
                  )}
                  {asrResult && (
                    <p className="mt-2 text-xs text-slate-600">
                      {asrResult.language} ·{" "}
                      {formatAsrDebugTime(asrResult.audioDurationSec)} audio · RTF{" "}
                      {asrResult.rtf.toFixed(2)}
                      {asrResult.tokensGenerated
                        ? ` · ${asrResult.tokensGenerated} tokens`
                        : ""}
                    </p>
                  )}
                </div>
                <div className="max-h-56 overflow-auto rounded-lg border border-slate-200 bg-white">
                  {(asrEvents.length > 0 ? asrEvents : asrResult?.events ?? []).map((event, index) => (
                    <div
                      className="border-b border-slate-100 p-3 last:border-b-0"
                      key={`${event.kind}-${index}`}
                    >
                      <div className="mb-1 flex items-center justify-between gap-2 text-[11px] font-extrabold uppercase text-slate-500">
                        <span>{event.kind}</span>
                        <span>{formatAsrDebugTime(event.endTimeSec)}</span>
                      </div>
                      <p className="whitespace-pre-wrap text-sm font-semibold text-slate-900">
                        {event.text || "(empty)"}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
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
              {autoTtsStatus?.lastSkipReason
                ? `: ${autoTtsStatus.lastSkipReason}`
                : ""}
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
