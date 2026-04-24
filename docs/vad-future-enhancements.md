# VAD Voice Recorder - Future Enhancements

## 9.1 Configurable VAD Parameters UI

Allow users to adjust VAD engine parameters at runtime.

- **Threshold slider (0.0–1.0)**: Lower values detect quieter speech, higher values reduce false positives
- **Silence duration input**: Control how long silence must last before recording stops (default 480ms / 30 frames)
- **Hop size selector**: Trade off between latency and accuracy (default 256 = 16ms)

Relevant code: `src/hooks/useVAD.ts` constants at the top of the file (`HOP_SIZE`, `THRESHOLD`, `SILENCE_FRAMES`).

---

## 9.2 Pre-buffer to Reduce Audio Loss

Implement a circular pre-buffer to capture the ~32ms of audio that is lost when VAD transitions from listening to recording.

- Maintain a 500ms (8000 samples) ring buffer during the `listening` state
- On speech detection, prepend the pre-buffer contents to the recording buffer
- Trade-off: slightly higher memory usage (~16KB) for no audio loss at speech onset

Relevant code: `src/hooks/useVAD.ts` → `processFrame`, the `bufferRef` and state transition from `listening` to `recording`.

---

## 9.3 Audio Waveform Visualization

Add real-time audio waveform display during recording.

- **Real-time waveform**: Draw the audio amplitude using a `<canvas>` element
- **Frequency analysis**: Use `AnalyserNode` from Web Audio API for FFT-based frequency display
- Can be integrated into `src/components/AudioVisualizer.tsx` (currently only shows status text)

Relevant code: `src/components/AudioVisualizer.tsx`, `src/hooks/useVAD.ts` → `audioContextRef` / `sourceRef`.

---

## 9.4 Transcription History

Persist and manage past transcriptions.

- **Store recent transcriptions**: Keep a list with timestamps in `localStorage`
- **Export to file**: Allow saving transcription history as `.txt` or `.json`
- **Search/filter**: Add a search bar to filter through past transcriptions
- **Clear history**: Button to delete all stored transcriptions

Relevant code: `src/components/VoiceRecorder.tsx` → `transcriptHistoryRef` (currently in-memory only, resets on page reload).

---

## Notes

These enhancements are independent and can be implemented in any order. Each one is self-contained and does not block the others.
