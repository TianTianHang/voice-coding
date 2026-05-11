struct MossStreamPrepared {
    assets: MossAssets,
    prompt_audio_codes: Vec<Vec<i64>>,
    sampling_mode: MossSamplingMode,
    reference_audio: Option<ReferenceAudio>,
    requested_chunk_ms: Option<u32>,
}

struct StreamWorkerChunk {
    chunk: PreparedTextChunk,
    is_final: bool,
}

struct MossStreamSession<'a> {
    engine: &'a MossOnnxTtsEngine,
    config: TtsConfig,
    buffer: String,
    absolute_offset: usize,
    queued_chunks: Vec<StreamWorkerChunk>,
    chunks_tx: Option<std::sync::mpsc::Sender<Option<StreamWorkerChunk>>>,
    events_rx: Option<tokio::sync::mpsc::UnboundedReceiver<TtsSynthesisEvent>>,
    worker: Option<tokio::task::JoinHandle<tts_core::Result<TtsResult>>>,
    cancel_flag: Arc<AtomicBool>,
    final_result: Option<TtsResult>,
    finished: bool,
    started: bool,
}

impl<'a> MossStreamSession<'a> {
    fn new(engine: &'a MossOnnxTtsEngine, config: TtsConfig) -> Self {
        Self {
            engine,
            config,
            buffer: String::new(),
            absolute_offset: 0,
            queued_chunks: Vec::new(),
            chunks_tx: None,
            events_rx: None,
            worker: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            final_result: None,
            finished: false,
            started: false,
        }
    }

    fn should_commit(&self, chunk: &StreamingTextChunk) -> bool {
        if chunk.flush || chunk.is_final {
            return true;
        }
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() {
            return false;
        }
        let stream = self.config.stream.as_ref();
        let min_chars = stream
            .and_then(|config| config.min_text_chunk_chars)
            .unwrap_or(24);
        let max_chars = stream
            .and_then(|config| config.max_buffered_text_chars)
            .unwrap_or(240);
        if trimmed.chars().count() >= max_chars {
            return true;
        }
        let flush_on_punctuation = stream
            .and_then(|config| config.flush_on_punctuation)
            .unwrap_or(true);
        flush_on_punctuation
            && trimmed.chars().count() >= min_chars
            && trimmed.chars().last().is_some_and(is_stream_boundary)
    }

    fn commit_buffer(&mut self, is_final: bool) -> tts_core::Result<()> {
        let raw = std::mem::take(&mut self.buffer);
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            self.absolute_offset = self.absolute_offset.saturating_add(raw.len());
            return Ok(());
        }
        let prefix_trim = raw.len() - raw.trim_start().len();
        let start = self.absolute_offset + prefix_trim;
        self.absolute_offset = self.absolute_offset.saturating_add(raw.len());
        let prepared = self
            .engine
            .text_preprocessor
            .prepare(trimmed, |text| self.engine.tokenizer.encode_ids(text))?;
        for chunk in prepared {
            self.queue_or_send_chunk(StreamWorkerChunk {
                chunk: PreparedTextChunk {
                    text: chunk.text,
                    token_ids: chunk.token_ids,
                },
                is_final,
            })?;
        }
        if start > 0 && self.queued_chunks.is_empty() {
            self.absolute_offset = self.absolute_offset.max(start);
        }
        Ok(())
    }

    fn queue_or_send_chunk(&mut self, chunk: StreamWorkerChunk) -> tts_core::Result<()> {
        if let Some(sender) = &self.chunks_tx {
            sender.send(Some(chunk)).map_err(|_| {
                TtsError::SynthesisError("MOSS streaming worker stopped accepting text".to_string())
            })?;
        } else {
            self.queued_chunks.push(chunk);
        }
        Ok(())
    }

    fn ensure_worker_started(&mut self) -> tts_core::Result<()> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        let sampling_mode = MossSamplingMode::from_config(&self.config)?;
        let voice = self.engine.assets.resolve_voice(self.config.voice.as_deref())?;
        let reference_audio = reference_audio_path(&self.config)
            .map(|path| ReferenceAudio::from_wav_path(&path))
            .transpose()?;
        let prepared = MossStreamPrepared {
            assets: self.engine.assets.clone(),
            prompt_audio_codes: voice.prompt_audio_codes.clone(),
            sampling_mode,
            reference_audio,
            requested_chunk_ms: self
                .config
                .stream
                .as_ref()
                .and_then(|stream| stream.audio_chunk_ms),
        };
        let sessions = Arc::clone(&self.engine.sessions);
        let cancel_flag = Arc::clone(&self.cancel_flag);
        let (events_tx, events_rx) = tokio::sync::mpsc::unbounded_channel();
        let (chunks_tx, chunks_rx) = std::sync::mpsc::channel();
        for chunk in std::mem::take(&mut self.queued_chunks) {
            chunks_tx.send(Some(chunk)).map_err(|_| {
                TtsError::SynthesisError(
                    "MOSS streaming worker stopped during startup".to_string(),
                )
            })?;
        }
        self.chunks_tx = Some(chunks_tx);
        self.events_rx = Some(events_rx);
        self.worker = Some(tokio::task::spawn_blocking(move || {
            synthesize_stream_prepared_with_sessions(
                &sessions,
                prepared,
                chunks_rx,
                cancel_flag,
                events_tx,
            )
            .map_err(TtsError::from)
        }));
        Ok(())
    }
}

#[async_trait::async_trait]
impl StreamingTts for MossOnnxTtsEngine {
    async fn start_stream<'stream>(
        &'stream self,
        config: TtsConfig,
    ) -> tts_core::Result<Box<dyn StreamingTtsSession + Send + 'stream>> {
        Ok(Box::new(MossStreamSession::new(self, config)))
    }
}

#[async_trait::async_trait]
impl<'stream> StreamingTtsSession for MossStreamSession<'stream> {
    async fn push_text(&mut self, chunk: StreamingTextChunk) -> tts_core::Result<()> {
        if self.finished || self.started {
            return Err(TtsError::InvalidInput(
                "cannot push text after stream processing has started".to_string(),
            ));
        }
        self.buffer.push_str(&chunk.text);
        if self.should_commit(&chunk) {
            self.commit_buffer(chunk.is_final)?;
            if !self.queued_chunks.is_empty() {
                self.ensure_worker_started()?;
            }
        }
        if chunk.is_final {
            self.finished = true;
            if !self.buffer.trim().is_empty() {
                self.commit_buffer(true)?;
            }
            if self.queued_chunks.is_empty() && !self.started {
                return Err(TtsError::InvalidInput(
                    "text produced no speakable MOSS chunks".to_string(),
                ));
            }
            self.ensure_worker_started()?;
            if let Some(sender) = self.chunks_tx.take() {
                sender.send(None).map_err(|_| {
                    TtsError::SynthesisError(
                        "MOSS streaming worker stopped before final flush".to_string(),
                    )
                })?;
            }
        }
        Ok(())
    }

    async fn next_event(&mut self) -> tts_core::Result<Option<TtsSynthesisEvent>> {
        if let Some(receiver) = &mut self.events_rx {
            match receiver.try_recv() {
                Ok(event) => return Ok(Some(event)),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return Ok(None),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {}
            }
        }
        Ok(None)
    }

    async fn finish(&mut self) -> tts_core::Result<TtsResult> {
        if let Some(result) = self.final_result.clone() {
            return Ok(result);
        }
        if !self.buffer.trim().is_empty() {
            self.commit_buffer(true)?;
        }
        self.finished = true;
        if self.queued_chunks.is_empty() && !self.started {
            return Err(TtsError::InvalidInput(
                "text produced no speakable MOSS chunks".to_string(),
            ));
        }
        self.ensure_worker_started()?;
        if let Some(sender) = self.chunks_tx.take() {
            sender.send(None).map_err(|_| {
                TtsError::SynthesisError("MOSS streaming worker stopped before finish".to_string())
            })?;
        }
        let worker = self.worker.take().ok_or_else(|| {
            TtsError::SynthesisError("MOSS streaming worker was not started".to_string())
        })?;
        let result = worker.await.map_err(|e| {
            TtsError::SynthesisError(format!("MOSS streaming worker join failed: {e}"))
        })??;
        self.final_result = Some(result.clone());
        Ok(result)
    }

    async fn cancel(&mut self) -> tts_core::Result<()> {
        self.cancel_flag.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
        self.chunks_tx.take();
        if let Some(receiver) = &mut self.events_rx {
            while receiver.try_recv().is_ok() {}
        }
        self.finished = true;
        Ok(())
    }
}

fn synthesize_stream_prepared_with_sessions(
    sessions: &Mutex<Option<MossSessions>>,
    prepared: MossStreamPrepared,
    chunks_rx: std::sync::mpsc::Receiver<Option<StreamWorkerChunk>>,
    cancel_flag: Arc<AtomicBool>,
    events_tx: tokio::sync::mpsc::UnboundedSender<TtsSynthesisEvent>,
) -> Result<TtsResult, MossTtsError> {
    if cancel_flag.load(Ordering::SeqCst) {
        return Err(cancelled_stream_error());
    }
    send_stream_event(
        &cancel_flag,
        &events_tx,
        TtsSynthesisEvent::Started(TtsSynthesisStarted { text: None }),
    )?;

    let mut sessions = sessions.lock().map_err(|e| MossTtsError::Inference {
        stage: "session_lock",
        detail: e.to_string(),
    })?;
    if sessions.is_none() {
        *sessions = Some(MossSessions::load(&prepared.assets)?);
    }
    let sessions = sessions.as_mut().ok_or_else(|| MossTtsError::Inference {
        stage: "session_init",
        detail: "MOSS sessions were not initialized".to_string(),
    })?;
    let prompt_audio_codes = if let Some(reference_audio) = prepared.reference_audio {
        sessions.encode_reference_audio(reference_audio)?
    } else {
        prepared.prompt_audio_codes
    };
    synthesize_stream_chunks(
        chunks_rx,
        cancel_flag,
        events_tx,
        |chunk, emit_chunk| {
            let request = prepared
                .assets
                .build_voice_clone_request_rows(chunk.token_ids.clone(), &prompt_audio_codes)?;
            sessions.synthesize_streaming_frames(
                &prepared.assets,
                request,
                prepared.sampling_mode,
                prepared.requested_chunk_ms,
                emit_chunk,
            )
        },
    )
}

fn synthesize_stream_chunks<F>(
    chunks_rx: std::sync::mpsc::Receiver<Option<StreamWorkerChunk>>,
    cancel_flag: Arc<AtomicBool>,
    events_tx: tokio::sync::mpsc::UnboundedSender<TtsSynthesisEvent>,
    mut synthesize_chunk: F,
) -> Result<TtsResult, MossTtsError>
where
    F: FnMut(
        &PreparedTextChunk,
        &mut dyn FnMut(Vec<f32>, bool) -> Result<(), MossTtsError>,
    ) -> Result<TtsResult, MossTtsError>,
{
    let mut results = Vec::new();
    let mut sequence = 0u64;
    let mut produced_samples = 0usize;
    let mut produced_chunks = 0usize;
    while let Some(worker_chunk) = recv_stream_chunk(&chunks_rx, &cancel_flag)? {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(cancelled_stream_error());
        }
        produced_chunks = produced_chunks.saturating_add(1);
        let StreamWorkerChunk { chunk, is_final } = worker_chunk;
        send_stream_event(
            &cancel_flag,
            &events_tx,
            TtsSynthesisEvent::TextBoundary(TtsTextBoundary {
                text: chunk.text.clone(),
                start: 0,
                end: chunk.text.len(),
                is_final,
            }),
        )?;
        let mut emit_chunk = |samples, is_final_batch| {
            if cancel_flag.load(Ordering::SeqCst) {
                return Err(cancelled_stream_error());
            }
            sequence = sequence.saturating_add(1);
            let is_final = is_final && is_final_batch;
            send_stream_event(
                &cancel_flag,
                &events_tx,
                make_audio_chunk_event(
                    sequence,
                    samples,
                    &mut produced_samples,
                    chunk.text.len(),
                    is_final,
                ),
            )
        };
        let result = synthesize_chunk(&chunk, &mut emit_chunk)?;
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(cancelled_stream_error());
        }
        results.push(result);
        send_stream_event(
            &cancel_flag,
            &events_tx,
            TtsSynthesisEvent::Progress(TtsSynthesisProgress {
                stage: "moss_streaming".to_string(),
                produced_chunks,
                total_chunks_hint: None,
            }),
        )?;
    }
    if results.is_empty() {
        return Err(MossTtsError::Inference {
            stage: "moss_streaming",
            detail: "text produced no speakable MOSS chunks".to_string(),
        });
    }

    let result = concatenate_tts_results(results)?;
    send_stream_event(
        &cancel_flag,
        &events_tx,
        TtsSynthesisEvent::End(result.clone()),
    )?;
    Ok(result)
}

fn recv_stream_chunk(
    chunks_rx: &std::sync::mpsc::Receiver<Option<StreamWorkerChunk>>,
    cancel_flag: &AtomicBool,
) -> Result<Option<StreamWorkerChunk>, MossTtsError> {
    while !cancel_flag.load(Ordering::SeqCst) {
        match chunks_rx.recv_timeout(std::time::Duration::from_millis(20)) {
            Ok(chunk) => return Ok(chunk),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
    Err(cancelled_stream_error())
}

fn send_stream_event(
    cancel_flag: &AtomicBool,
    events_tx: &tokio::sync::mpsc::UnboundedSender<TtsSynthesisEvent>,
    event: TtsSynthesisEvent,
) -> Result<(), MossTtsError> {
    if cancel_flag.load(Ordering::SeqCst) {
        return Err(cancelled_stream_error());
    }
    events_tx.send(event).map_err(|_| MossTtsError::Inference {
        stage: "moss_streaming",
        detail: "stream event receiver was dropped".to_string(),
    })
}

fn cancelled_stream_error() -> MossTtsError {
    MossTtsError::Inference {
        stage: "moss_streaming",
        detail: "streaming synthesis was cancelled".to_string(),
    }
}

fn is_stream_boundary(ch: char) -> bool {
    matches!(
        ch,
        '.' | '!' | '?' | ';' | ':' | ',' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}' | '\u{ff0c}'
    )
}

fn make_audio_chunk_event(
    sequence: u64,
    samples: Vec<f32>,
    produced_samples: &mut usize,
    text_len: usize,
    is_final: bool,
) -> TtsSynthesisEvent {
    let frame_samples = samples.len() / PLAYBACK_CHANNELS as usize;
    let start = *produced_samples / PLAYBACK_CHANNELS as usize;
    *produced_samples = produced_samples.saturating_add(samples.len());
    let end = start.saturating_add(frame_samples);
    TtsSynthesisEvent::AudioChunk(TtsAudioChunk {
        sequence,
        audio: AudioBuffer {
            sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
            channels: PLAYBACK_CHANNELS,
            pcm: PcmData::F32(samples),
        },
        start_time_sec: Some(start as f64 / PLAYBACK_SAMPLE_RATE_HZ as f64),
        end_time_sec: Some(end as f64 / PLAYBACK_SAMPLE_RATE_HZ as f64),
        text_start: Some(0),
        text_end: Some(text_len),
        is_final,
    })
}
