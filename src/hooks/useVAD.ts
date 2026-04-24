import { useRef, useCallback, useState } from "react";
import createVADModule from "../lib/ten_vad.js";
import type { TenVADModule } from "../lib/ten_vad";

const HOP_SIZE = 256;
const SAMPLE_RATE = 16000;
const THRESHOLD = 0.5;
const SILENCE_FRAMES = 30;
const MAX_RECORDING_SECONDS = 30;

export type VADState = "idle" | "listening" | "recording" | "processing";
export type { VADState as VADStateType };

/**
 * VAD state machine:
 *   idle → (startListening) → listening → (speech detected) → recording
 *   recording → (SILENCE_FRAMES silence) → processing → (transcription done) → idle
 *   any → (stopListening / error) → idle
 */

export interface VADResult {
  state: VADState;
  error: string | null;
  startListening: () => Promise<void>;
  stopListening: () => void;
  recordingDuration: number;
}

export interface VADCallbacks {
  onRecordingStart: () => void;
  onRecordingStop: (audioData: Int16Array) => void;
  onStateChange: (state: VADState) => void;
  onError: (error: string) => void;
}

function addHelperFunctions(mod: TenVADModule) {
  if (!mod.getValue) {
    mod.getValue = function (ptr: number, type: string) {
      const buffer = mod.HEAPU8.buffer;
      const view = new DataView(buffer);
      switch (type) {
        case "i32":
          return view.getInt32(ptr, true);
        case "float":
          return view.getFloat32(ptr, true);
        default:
          throw new Error(`Unsupported type: ${type}`);
      }
    };
  }

  if (!mod.UTF8ToString) {
    mod.UTF8ToString = function (ptr: number) {
      if (!ptr) return "";
      const HEAPU8 = mod.HEAPU8;
      let endPtr = ptr;
      while (HEAPU8[endPtr]) ++endPtr;
      const bytes = HEAPU8.subarray(ptr, endPtr);
      return new TextDecoder("utf-8").decode(bytes);
    };
  }
}

export function useVAD(callbacks: VADCallbacks): VADResult {
  const [state, setState] = useState<VADState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [recordingDuration, setRecordingDuration] = useState(0);

  const vadModuleRef = useRef<TenVADModule | null>(null);
  const vadHandleRef = useRef<number | null>(null);
  const vadHandlePtrRef = useRef<number | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const scriptProcessorRef = useRef<ScriptProcessorNode | null>(null);
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const bufferRef = useRef<Int16Array[]>([]);
  const silenceCounterRef = useRef(0);
  const isActiveRef = useRef(false);
  const recordingStartRef = useRef(0);
  const durationTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const stateRef = useRef<VADState>("idle");

  const updateState = useCallback(
    (newState: VADState) => {
      stateRef.current = newState;
      setState(newState);
      callbacks.onStateChange(newState);
    },
    [callbacks]
  );

  const cleanup = useCallback(() => {
    isActiveRef.current = false;

    if (durationTimerRef.current) {
      clearInterval(durationTimerRef.current);
      durationTimerRef.current = null;
    }

    if (scriptProcessorRef.current) {
      scriptProcessorRef.current.disconnect();
      scriptProcessorRef.current = null;
    }

    if (sourceRef.current) {
      sourceRef.current.disconnect();
      sourceRef.current = null;
    }

    if (
      vadHandlePtrRef.current !== null &&
      vadModuleRef.current
    ) {
      try {
        vadModuleRef.current._ten_vad_destroy(vadHandlePtrRef.current);
        vadModuleRef.current._free(vadHandlePtrRef.current);
      } catch {
        // ignore cleanup errors
      }
      vadHandleRef.current = null;
      vadHandlePtrRef.current = null;
    }

    if (audioContextRef.current) {
      audioContextRef.current.close().catch(() => {});
      audioContextRef.current = null;
    }

    if (mediaStreamRef.current) {
      mediaStreamRef.current.getTracks().forEach((t) => t.stop());
      mediaStreamRef.current = null;
    }

    bufferRef.current = [];
    silenceCounterRef.current = 0;
    setRecordingDuration(0);
  }, []);

  const processFrame = useCallback(
    (frameData: Int16Array) => {
      const mod = vadModuleRef.current;
      const handle = vadHandleRef.current;
      if (!mod || handle === null || !isActiveRef.current) return;

      const audioPtr = mod._malloc(HOP_SIZE * 2);
      const probPtr = mod._malloc(4);
      const flagPtr = mod._malloc(4);

      try {
        mod.HEAP16.set(frameData, audioPtr / 2);

        const result = mod._ten_vad_process(
          handle,
          audioPtr,
          HOP_SIZE,
          probPtr,
          flagPtr
        );

        if (result !== 0) return;

        const flag = mod.getValue(flagPtr, "i32");
        const isSpeech = flag === 1;
        const currentState = stateRef.current;

        if (currentState === "listening" && isSpeech) {
          updateState("recording");
          bufferRef.current = [];
          recordingStartRef.current = Date.now();
          silenceCounterRef.current = 0;
          callbacks.onRecordingStart();

          durationTimerRef.current = setInterval(() => {
            const elapsed = (Date.now() - recordingStartRef.current) / 1000;
            setRecordingDuration(elapsed);
          }, 100);
        }

        if (currentState === "recording" || stateRef.current === "recording") {
          if (stateRef.current !== "recording") return;

          bufferRef.current.push(new Int16Array(frameData));

          const maxFrames =
            (MAX_RECORDING_SECONDS * SAMPLE_RATE) / HOP_SIZE;
          if (bufferRef.current.length > maxFrames) {
            bufferRef.current = bufferRef.current.slice(-maxFrames);
          }

          if (isSpeech) {
            silenceCounterRef.current = 0;
          } else {
            silenceCounterRef.current++;
            if (silenceCounterRef.current >= SILENCE_FRAMES) {
              if (durationTimerRef.current) {
                clearInterval(durationTimerRef.current);
                durationTimerRef.current = null;
              }

              const totalSamples = bufferRef.current.reduce(
                (sum, buf) => sum + buf.length,
                0
              );
              const audioData = new Int16Array(totalSamples);
              let offset = 0;
              for (const chunk of bufferRef.current) {
                audioData.set(chunk, offset);
                offset += chunk.length;
              }

              bufferRef.current = [];
              silenceCounterRef.current = 0;
              updateState("processing");
              callbacks.onRecordingStop(audioData);
            }
          }
        }
      } finally {
        mod._free(audioPtr);
        mod._free(probPtr);
        mod._free(flagPtr);
      }
    },
    [callbacks, updateState]
  );

  const startListening = useCallback(async () => {
    try {
      setError(null);
      updateState("listening");

      const mod = await createVADModule();
      addHelperFunctions(mod);
      vadModuleRef.current = mod;

      const handlePtr = mod._malloc(4);
      const createResult = mod._ten_vad_create(handlePtr, HOP_SIZE, THRESHOLD);

      if (createResult !== 0) {
        mod._free(handlePtr);
        throw new Error("VAD initialization failed");
      }

      vadHandlePtrRef.current = handlePtr;
      vadHandleRef.current = mod.getValue(handlePtr, "i32");

      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          channelCount: 1,
          sampleRate: SAMPLE_RATE,
          echoCancellation: true,
          noiseSuppression: true,
        },
      });
      mediaStreamRef.current = stream;

      const audioCtx = new AudioContext({ sampleRate: SAMPLE_RATE });
      audioContextRef.current = audioCtx;

      const source = audioCtx.createMediaStreamSource(stream);
      sourceRef.current = source;

      const processor = audioCtx.createScriptProcessor(HOP_SIZE, 1, 1);
      scriptProcessorRef.current = processor;

      processor.onaudioprocess = (event: AudioProcessingEvent) => {
        if (!isActiveRef.current) return;

        const inputBuffer = event.inputBuffer;
        const float32Data = inputBuffer.getChannelData(0);
        const int16Data = new Int16Array(HOP_SIZE);

        for (let i = 0; i < HOP_SIZE; i++) {
          const s = Math.max(-1, Math.min(1, float32Data[i]));
          int16Data[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
        }

        processFrame(int16Data);
      };

      source.connect(processor);
      processor.connect(audioCtx.destination);

      isActiveRef.current = true;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to start listening";
      setError(message);
      callbacks.onError(message);
      cleanup();
      updateState("idle");
    }
  }, [callbacks, cleanup, processFrame, updateState]);

  const stopListening = useCallback(() => {
    cleanup();
    updateState("idle");
  }, [cleanup, updateState]);

  return {
    state,
    error,
    startListening,
    stopListening,
    recordingDuration,
  };
}
