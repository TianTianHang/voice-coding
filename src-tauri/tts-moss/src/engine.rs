pub struct MossOnnxTtsEngine {
    assets: MossAssets,
    tokenizer: MossTokenizer,
    text_preprocessor: MossTextPreprocessor,
    sessions: Arc<Mutex<Option<MossSessions>>>,
}

struct PreparedSynthesis {
    assets: MossAssets,
    prompt_audio_codes: Vec<Vec<i64>>,
    chunks: Vec<PreparedTextChunk>,
    sampling_mode: MossSamplingMode,
    generation_config: MossGenerationConfig,
    reference_audio: Option<ReferenceAudio>,
}

enum MossTokenizer {
    SentencePiece(SentencePieceProcessor),
    HuggingFace(Box<tokenizers::Tokenizer>),
}

impl MossTokenizer {
    fn load(path: &Path) -> Result<Self, MossTtsError> {
        if path.extension().and_then(|extension| extension.to_str()) == Some("model") {
            return SentencePieceProcessor::open(path)
                .map(Self::SentencePiece)
                .map_err(|e| MossTtsError::Tokenizer(e.to_string()));
        }

        tokenizers::Tokenizer::from_file(path)
            .map(|tokenizer| Self::HuggingFace(Box::new(tokenizer)))
            .map_err(|e| MossTtsError::Tokenizer(e.to_string()))
    }

    fn encode_ids(&self, text: &str) -> Result<Vec<i64>, MossTtsError> {
        match self {
            Self::SentencePiece(tokenizer) => tokenizer
                .encode(text)
                .map(|pieces| pieces.into_iter().map(|piece| piece.id as i64).collect())
                .map_err(|e| MossTtsError::Tokenizer(e.to_string())),
            Self::HuggingFace(tokenizer) => tokenizer
                .encode(text, false)
                .map(|encoding| encoding.get_ids().iter().map(|id| *id as i64).collect())
                .map_err(|e| MossTtsError::Tokenizer(e.to_string())),
        }
    }
}

impl MossOnnxTtsEngine {
    pub fn from_env() -> Result<Self, MossTtsError> {
        Self::new(MossModelConfig::from_env())
    }

    pub fn new(config: MossModelConfig) -> Result<Self, MossTtsError> {
        let assets = MossAssets::load(config)?;
        let tokenizer = MossTokenizer::load(&assets.tokenizer_path)?;
        Ok(Self {
            assets,
            tokenizer,
            text_preprocessor: MossTextPreprocessor::default(),
            sessions: Arc::new(Mutex::new(None)),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn validate_output_contract(&self, result: &TtsResult) -> Result<(), MossTtsError> {
        if result.audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ {
            return Err(MossTtsError::OutputFormat(format!(
                "expected {}Hz, got {}Hz",
                PLAYBACK_SAMPLE_RATE_HZ, result.audio.sample_rate_hz
            )));
        }
        if result.audio.channels != PLAYBACK_CHANNELS {
            return Err(MossTtsError::OutputFormat(format!(
                "expected {} channels, got {}",
                PLAYBACK_CHANNELS, result.audio.channels
            )));
        }
        result
            .audio
            .validate()
            .map_err(|e| MossTtsError::OutputFormat(e.to_string()))
    }

    #[cfg(test)]
    fn from_assets_for_test(assets: MossAssets) -> Self {
        let tokenizer = MossTokenizer::HuggingFace(Box::new(tokenizers::Tokenizer::new(
            tokenizers::models::bpe::BPE::default(),
        )));
        Self {
            assets,
            tokenizer,
            text_preprocessor: MossTextPreprocessor::default(),
            sessions: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl TtsEngine for MossOnnxTtsEngine {
    fn engine_name(&self) -> &str {
        "moss-onnx-tts"
    }

    async fn synthesize(&self, text: &str, config: TtsConfig) -> tts_core::Result<TtsResult> {
        if text.trim().is_empty() {
            return Err(TtsError::InvalidInput("text must not be empty".to_string()));
        }
        let sampling_mode = MossSamplingMode::from_config(&config)?;
        let voice = self.assets.resolve_voice(config.voice.as_deref())?;
        let reference_audio = reference_audio_path(&config)
            .map(|path| ReferenceAudio::from_wav_path(&path))
            .transpose()?;
        let chunks = self
            .text_preprocessor
            .prepare(text, |chunk| self.tokenizer.encode_ids(chunk))?;
        if chunks.is_empty() {
            return Err(TtsError::InvalidInput(
                "text produced no speakable MOSS chunks".to_string(),
            ));
        }

        let prepared = PreparedSynthesis {
            assets: self.assets.clone(),
            prompt_audio_codes: voice.prompt_audio_codes.clone(),
            chunks,
            sampling_mode,
            generation_config: MossGenerationConfig::from_tts_config(&config),
            reference_audio,
        };
        let sessions = Arc::clone(&self.sessions);
        let result = tokio::task::spawn_blocking(move || {
            synthesize_prepared_with_sessions(&sessions, prepared)
        })
        .await
        .map_err(|e| MossTtsError::Inference {
            stage: "inference_worker",
            detail: e.to_string(),
        })??;
        self.validate_output_contract(&result)?;
        Ok(result)
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        config: TtsConfig,
    ) -> tts_core::Result<Vec<TtsSynthesisEvent>> {
        let mut events = Vec::new();
        self.synthesize_stream_events(text, config, Box::new(|event| events.push(event)))
            .await?;
        Ok(events)
    }

    async fn synthesize_stream_events(
        &self,
        text: &str,
        config: TtsConfig,
        mut on_event: Box<dyn FnMut(TtsSynthesisEvent) + Send + 'async_trait>,
    ) -> tts_core::Result<TtsResult> {
        let mut session = self.start_stream(config).await?;
        session
            .push_text(StreamingTextChunk::final_chunk(text))
            .await?;
        let mut emitted_end = false;
        loop {
            match session.next_event().await? {
                Some(event) => {
                    emitted_end |= matches!(event, TtsSynthesisEvent::End(_));
                    on_event(event);
                    if emitted_end {
                        break;
                    }
                }
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            }
        }
        let result = session.finish().await?;
        if !emitted_end {
            on_event(TtsSynthesisEvent::End(result.clone()));
        }
        Ok(result)
    }

    async fn health_check(&self) -> tts_core::Result<bool> {
        let sessions = Arc::clone(&self.sessions);
        tokio::task::spawn_blocking(move || {
            let mut sessions = sessions.lock().map_err(|e| MossTtsError::Inference {
                stage: "session_lock",
                detail: e.to_string(),
            })?;
            if sessions.is_none() {
                *sessions = Some(MossSessions::new());
            }
            Ok::<(), MossTtsError>(())
        })
        .await
        .map_err(|e| MossTtsError::Inference {
            stage: "inference_worker",
            detail: e.to_string(),
        })??;
        Ok(true)
    }
}

fn synthesize_prepared_with_sessions(
    sessions: &Mutex<Option<MossSessions>>,
    prepared: PreparedSynthesis,
) -> Result<TtsResult, MossTtsError> {
    let mut sessions = sessions.lock().map_err(|e| MossTtsError::Inference {
        stage: "session_lock",
        detail: e.to_string(),
    })?;
    if sessions.is_none() {
        *sessions = Some(MossSessions::new());
    }
    let sessions = sessions.as_mut().ok_or_else(|| MossTtsError::Inference {
        stage: "session_init",
        detail: "MOSS sessions were not initialized".to_string(),
    })?;
    let prompt_audio_codes = if let Some(reference_audio) = prepared.reference_audio {
        sessions.encode_reference_audio(&prepared.assets, reference_audio)?
    } else {
        prepared.prompt_audio_codes
    };
    synthesize_chunks(
        sessions,
        &prepared.assets,
        &prompt_audio_codes,
        prepared.chunks,
        prepared.sampling_mode,
        prepared.generation_config,
    )
}

fn synthesize_chunks(
    sessions: &mut MossSessions,
    assets: &MossAssets,
    prompt_audio_codes: &[Vec<i64>],
    chunks: Vec<PreparedTextChunk>,
    sampling_mode: MossSamplingMode,
    generation_config: MossGenerationConfig,
) -> Result<TtsResult, MossTtsError> {
    let mut results = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let request = assets.build_voice_clone_request_rows(chunk.token_ids, prompt_audio_codes)?;
        results.push(sessions.synthesize(assets, request, sampling_mode, generation_config)?);
    }
    concatenate_tts_results(results)
}

fn concatenate_tts_results(results: Vec<TtsResult>) -> Result<TtsResult, MossTtsError> {
    let mut total_samples = 0usize;
    let pause_samples = pause_samples_between_chunks(results.len());
    for (index, result) in results.iter().enumerate() {
        if result.audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ
            || result.audio.channels != PLAYBACK_CHANNELS
        {
            return Err(MossTtsError::OutputFormat(format!(
                "chunk audio must be {}Hz stereo, got {}Hz/{}ch",
                PLAYBACK_SAMPLE_RATE_HZ, result.audio.sample_rate_hz, result.audio.channels
            )));
        }
        match &result.audio.pcm {
            PcmData::F32(samples) => {
                total_samples = total_samples.checked_add(samples.len()).ok_or_else(|| {
                    MossTtsError::OutputFormat("chunk PCM length overflowed usize".to_string())
                })?;
                if index + 1 < results.len() {
                    total_samples = total_samples.checked_add(pause_samples).ok_or_else(|| {
                        MossTtsError::OutputFormat("chunk pause PCM length overflowed usize".to_string())
                    })?;
                }
            }
            PcmData::I16(_) => {
                return Err(MossTtsError::OutputFormat(
                    "MOSS chunk audio must use f32 PCM".to_string(),
                ));
            }
        }
    }
    if total_samples == 0 {
        return Err(MossTtsError::Inference {
            stage: "tts_concat_chunks",
            detail: "MOSS chunks produced no audio".to_string(),
        });
    }
    let mut pcm = Vec::with_capacity(total_samples);
    let result_count = results.len();
    for (index, result) in results.into_iter().enumerate() {
        if let PcmData::F32(mut samples) = result.audio.pcm {
            pcm.append(&mut samples);
            if index + 1 < result_count {
                pcm.extend(std::iter::repeat_n(0.0, pause_samples));
            }
        }
    }
    Ok(TtsResult {
        audio: AudioBuffer {
            sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
            channels: PLAYBACK_CHANNELS,
            pcm: PcmData::F32(pcm),
        },
    })
}

fn pause_samples_between_chunks(chunk_count: usize) -> usize {
    if chunk_count > 1 {
        PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS as usize / 5
    } else {
        0
    }
}
