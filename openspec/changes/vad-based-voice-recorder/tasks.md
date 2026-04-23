# Implementation Tasks

## 1. Project Setup

- [ ] 1.1 Copy ten-vad WASM files from official repository to `src/lib/`
  - `ten_vad.js` (~5KB JavaScript glue code)
  - `ten_vad.wasm` (~278KB WebAssembly binary)
  - `ten_vad.d.ts` (TypeScript definitions)
- [ ] 1.2 Install frontend dependencies (if needed)
  - Verify React hooks compatibility
  - Check for additional audio processing libraries

## 2. VAD Integration (useVAD Hook)

- [ ] 2.1 Create `src/hooks/useVAD.ts` hook
  - Implement VAD module initialization
  - Create VAD instance with default parameters
  - Handle module loading errors
- [ ] 2.2 Implement VAD frame processing logic
  - Allocate WASM memory for audio frames
  - Process frames through `_ten_vad_process`
  - Extract probability and flag results
- [ ] 2.3 Implement VAD state machine
  - IDLE → RECORDING transition (speech detected)
  - RECORDING → PROCESSING transition (silence detected)
  - Silence counter (30 frames = 480ms)
- [ ] 2.4 Add memory management
  - Proper cleanup in useEffect return
  - Free all allocated pointers
  - Destroy VAD instance on unmount
- [ ] 2.5 Add error handling
  - Microphone access denial
  - WASM initialization failure
  - Invalid audio frame data

## 3. Audio Recording (useAudioRecorder Hook)

- [ ] 3.1 Create `src/hooks/useAudioRecorder.ts` hook
  - Request microphone access with `navigator.mediaDevices.getUserMedia`
  - Create AudioContext with 16kHz sample rate
  - Set up ScriptProcessor or AudioWorklet
- [ ] 3.2 Implement audio frame capture
  - Capture 16ms audio frames (256 samples @ 16kHz)
  - Resample if needed (browser default → 16kHz)
  - Convert to Int16Array format
- [ ] 3.3 Implement in-memory audio buffer
  - Circular buffer with 30-second limit
  - Append frames during RECORDING state
  - Clear buffer after transcription
- [ ] 3.4 Implement WAV encoding
  - Create WAV header (44 bytes)
  - Encode PCM audio data
  - Handle empty buffer edge case
- [ ] 3.5 Add recorder state management
  - Integrate with VAD state machine
  - Start/stop recording based on VAD signals
  - Expose recording status and duration

## 4. Transcription Integration (useTranscription Hook)

- [ ] 4.1 Create `src/hooks/useTranscription.ts` hook
  - Wrap Tauri `invoke` calls
  - Handle async transcription requests
- [ ] 4.2 Add Tauri backend command `transcribe_audio_data`
  - Accept `Vec<u8>` audio data
  - Save to temporary file with UUID
  - Call existing `transcribe` function
  - Clean up temporary file after transcription
- [ ] 4.3 Implement error handling
  - Transcription failure
  - Network/backend errors
  - Retry mechanism with user prompt
- [ ] 4.4 Add loading states
  - Show "Processing..." during transcription
  - Display results when complete
  - Handle empty results

## 5. UI Components

- [ ] 5.1 Create `src/components/VoiceRecorder.tsx` main component
  - Integrate useVAD, useAudioRecorder, useTranscription hooks
  - Manage overall recording flow
- [ ] 5.2 Create `src/components/AudioVisualizer.tsx` (optional)
  - Display recording status indicator
  - Show current state (Listening, Recording, Processing)
  - Color-coded feedback (blue, red, yellow)
- [ ] 5.3 Create `src/components/TranscriptDisplay.tsx`
  - Display transcribed text
  - Handle empty state
  - Support multiple transcription history
- [ ] 5.4 Create `src/components/ControlButton.tsx`
  - Start/Stop listening button
  - Error retry button
  - Disable during processing
- [ ] 5.5 Add responsive styling
  - CSS Modules or Tailwind classes
  - Mobile-friendly layout
  - Accessibility attributes

## 6. Integration

- [ ] 6.1 Update `src/App.tsx`
  - Import and render VoiceRecorder component
  - Set up error boundaries
- [ ] 6.2 Update Tauri `src-tauri/src/lib.rs`
  - Register new `transcribe_audio_data` command
  - Add to invoke handler
- [ ] 6.3 Implement `src-tauri/src/asr.rs` additions
  - Add `transcribe_audio_data` function
  - Implement temporary file management
  - Add UUID-based file naming
- [ ] 6.4 Update Tauri capabilities (if needed)
  - Add filesystem permissions for temp directory
  - Update `tauri.conf.json`

## 7. Testing

- [ ] 7.1 Unit tests for `useVAD` hook
  - Test VAD initialization
  - Test state machine transitions
  - Test memory cleanup
- [ ] 7.2 Unit tests for WAV encoding
  - Test header generation
  - Test data encoding
  - Test edge cases (empty buffer)
- [ ] 7.3 Integration tests
  - Test complete recording flow
  - Test error scenarios
  - Test microphone permission handling
- [ ] 7.4 Manual testing
  - Test with real microphone input
  - Test VAD detection accuracy
  - Test transcription quality
  - Performance profiling (CPU, memory)

## 8. Documentation and Polish

- [ ] 8.1 Add inline code comments
  - Explain VAD parameters
  - Document memory management
  - Clarify state transitions
- [ ] 8.2 Update README.md
  - Add VAD feature description
  - Document microphone permission requirements
  - Add troubleshooting section
- [ ] 8.3 Add user guidance
  - Permission request explanation
  - Error message clarity
  - Feature limitations disclosure
- [ ] 8.4 Performance optimization
  - Profile memory usage
  - Check for memory leaks
  - Optimize buffer size if needed

## 9. Optional Enhancements (Future)

- [ ] 9.1 Add configurable VAD parameters UI
  - Threshold slider (0.0-1.0)
  - Silence duration input
- [ ] 9.2 Implement pre-buffer to reduce audio loss
  - 500ms circular buffer
  - Include when recording starts
- [ ] 9.3 Add audio waveform visualization
  - Real-time waveform display
  - Frequency analysis
- [ ] 9.4 Add transcription history
  - Store recent transcriptions
  - Export to file
  - Search/filter functionality
