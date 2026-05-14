struct MossSessions {
    prefill: Option<Session>,
    decode_step: Option<Session>,
    local_fixed_sampled_frame: Option<Session>,
    local_decoder: Option<Session>,
    codec_encode: Option<Session>,
    codec_decode_full: Option<Session>,
    codec_decode_step: Option<Session>,
    codec_decode_step_state: Option<CodecDecodeStepState>,
}

impl MossSessions {
    fn new() -> Self {
        Self {
            prefill: None,
            decode_step: None,
            local_fixed_sampled_frame: None,
            local_decoder: None,
            codec_encode: None,
            codec_decode_full: None,
            codec_decode_step: None,
            codec_decode_step_state: None,
        }
    }

    fn end_ort_profiling(&mut self) {
        if let Some(session) = self.prefill.as_mut() {
            end_session_profiling(session, "tts.prefill");
        }
        if let Some(session) = self.decode_step.as_mut() {
            end_session_profiling(session, "tts.decode_step");
        }
        if let Some(session) = self.local_fixed_sampled_frame.as_mut() {
            end_session_profiling(session, "tts.local_fixed_sampled_frame");
        }
        if let Some(session) = self.local_decoder.as_mut() {
            end_session_profiling(session, "tts.local_decoder");
        }
        if let Some(session) = self.codec_encode.as_mut() {
            end_session_profiling(session, "codec.encode");
        }
        if let Some(session) = self.codec_decode_full.as_mut() {
            end_session_profiling(session, "codec.decode_full");
        }
        if let Some(session) = self.codec_decode_step.as_mut() {
            end_session_profiling(session, "codec.decode_step");
        }
    }

    fn ensure_prefill(&mut self, assets: &MossAssets) -> Result<&mut Session, MossTtsError> {
        if self.prefill.is_none() {
            let session = create_session(required_file(&assets.tts_files, "prefill")?, "tts.prefill")?;
            validate_session_io(
                &session,
                "tts.prefill",
                ["input_ids", "attention_mask"],
                assets.tts_meta.onnx.prefill_output_names.iter(),
            )?;
            self.prefill = Some(session);
        }
        Ok(self.prefill.as_mut().expect("prefill session initialized"))
    }

    fn ensure_decode_step(&mut self, assets: &MossAssets) -> Result<&mut Session, MossTtsError> {
        if self.decode_step.is_none() {
            let session = create_session(required_file(&assets.tts_files, "decode_step")?, "tts.decode_step")?;
            validate_session_io(
                &session,
                "tts.decode_step",
                assets.tts_meta.onnx.decode_input_names.iter(),
                assets.tts_meta.onnx.decode_output_names.iter(),
            )?;
            self.decode_step = Some(session);
        }
        Ok(self.decode_step.as_mut().expect("decode step session initialized"))
    }

    fn ensure_local_fixed_sampled_frame(
        &mut self,
        assets: &MossAssets,
    ) -> Result<&mut Session, MossTtsError> {
        if self.local_fixed_sampled_frame.is_none() {
            let session = create_session(
                required_file(&assets.tts_files, "local_fixed_sampled_frame")?,
                "tts.local_fixed_sampled_frame",
            )?;
            validate_session_io(
                &session,
                "tts.local_fixed_sampled_frame",
                &[
                    "global_hidden",
                    "repetition_seen_mask",
                    "assistant_random_u",
                    "audio_random_u",
                ],
                &["should_continue", "frame_token_ids"],
            )?;
            self.local_fixed_sampled_frame = Some(session);
        }
        Ok(self
            .local_fixed_sampled_frame
            .as_mut()
            .expect("fixed session initialized"))
    }

    fn ensure_local_decoder(&mut self, assets: &MossAssets) -> Result<&mut Session, MossTtsError> {
        if self.local_decoder.is_none() {
            let session = create_session(required_file(&assets.tts_files, "local_decoder")?, "tts.local_decoder")?;
            validate_session_io(
                &session,
                "tts.local_decoder",
                ["global_hidden", "text_token_id", "audio_prefix_token_ids"],
                ["text_logits", "audio_logits"],
            )?;
            self.local_decoder = Some(session);
        }
        Ok(self.local_decoder.as_mut().expect("local decoder initialized"))
    }

    fn ensure_codec_encode(&mut self, assets: &MossAssets) -> Result<&mut Session, MossTtsError> {
        if self.codec_encode.is_none() {
            let session = create_session(required_file(&assets.codec_files, "encode")?, "codec.encode")?;
            validate_session_io(
                &session,
                "codec.encode",
                assets.codec_meta.onnx.encode_input_names.iter(),
                assets.codec_meta.onnx.encode_output_names.iter(),
            )?;
            self.codec_encode = Some(session);
        }
        Ok(self.codec_encode.as_mut().expect("codec encode initialized"))
    }

    fn ensure_codec_decode_full(
        &mut self,
        assets: &MossAssets,
    ) -> Result<&mut Session, MossTtsError> {
        if self.codec_decode_full.is_none() {
            let session = create_session(
                required_file(&assets.codec_files, "decode_full")?,
                "codec.decode_full",
            )?;
            validate_session_io(
                &session,
                "codec.decode_full",
                assets.codec_meta.onnx.decode_input_names.iter(),
                assets.codec_meta.onnx.decode_output_names.iter(),
            )?;
            self.codec_decode_full = Some(session);
        }
        Ok(self
            .codec_decode_full
            .as_mut()
            .expect("codec full decode initialized"))
    }

    fn ensure_codec_decode_step(
        &mut self,
        assets: &MossAssets,
    ) -> Result<(&mut Session, CodecDecodeStepState), MossTtsError> {
        if self.codec_decode_step.is_none() {
            let session = create_session(
                required_file(&assets.codec_files, "decode_step")?,
                "codec.decode_step",
            )?;
            validate_session_io(
                &session,
                "codec.decode_step",
                assets.codec_meta.onnx.decode_step_input_names.iter(),
                assets.codec_meta.onnx.decode_step_output_names.iter(),
            )?;
            self.codec_decode_step = Some(session);
        }
        if self.codec_decode_step_state.is_none() {
            self.codec_decode_step_state = Some(CodecDecodeStepState::from_meta(&assets.codec_meta)?);
        }
        Ok((
            self.codec_decode_step
                .as_mut()
                .expect("codec step session initialized"),
            self.codec_decode_step_state
                .as_ref()
                .expect("codec step state initialized")
                .clone(),
        ))
    }

    fn encode_reference_audio(
        &mut self,
        assets: &MossAssets,
        audio: ReferenceAudio,
    ) -> Result<Vec<Vec<i64>>, MossTtsError> {
        if audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ || audio.channels != PLAYBACK_CHANNELS {
            return Err(MossTtsError::Inference {
                stage: "reference_audio",
                detail: "reference audio was not normalized to 48kHz stereo".to_string(),
            });
        }
        let frames = audio.samples.len() / PLAYBACK_CHANNELS as usize;
        let waveform =
            Array3::from_shape_fn((1, PLAYBACK_CHANNELS as usize, frames), |(_, channel, frame)| {
                audio.samples[frame * PLAYBACK_CHANNELS as usize + channel]
            });
        let input_lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);
        let outputs = self
            .ensure_codec_encode(assets)?
            .run(ort::inputs![
                "waveform" => TensorRef::from_array_view(waveform.view()).map_err(|e| MossTtsError::Inference { stage: "codec_encode", detail: e.to_string() })?,
                "input_lengths" => TensorRef::from_array_view(input_lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_encode", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_encode",
                detail: e.to_string(),
            })?;
        extract_audio_codes(&outputs, "codec_encode")
    }

    fn synthesize(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
        generation_config: MossGenerationConfig,
    ) -> Result<TtsResult, MossTtsError> {
        let total_started = Instant::now();
        let generated_frames =
            self.generate_audio_frames(assets, request, sampling_mode, generation_config)?;
        let frame_count = generated_frames.len();
        let decode_started = Instant::now();
        let result = self
            .decode_full_audio(assets, &generated_frames)
            .or_else(|full_error| {
                log::warn!(
                    target: "tts_moss::perf",
                    "codec_decode_full failed; falling back to codec_decode_step: {}",
                    full_error
                );
                self.decode_step_buffered(assets, &generated_frames).map_err(|step_error| {
                    MossTtsError::Inference {
                        stage: "codec_decode_step",
                        detail: format!(
                            "decode_full failed: {full_error}; decode_step fallback failed: {step_error}"
                        ),
                    }
                })
            });
        trace_moss_stage(
            "codec_decode_full_or_step",
            decode_started,
            format_args!("frames={frame_count}"),
        );
        let result = result?;
        trace_moss_stage(
            "synthesize_total",
            total_started,
            format_args!(
                "frames={} pcm_samples={}",
                frame_count,
                pcm_sample_count(&result.audio.pcm)
            ),
        );
        self.end_ort_profiling();
        Ok(result)
    }

    fn synthesize_streaming_frames<F>(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
        generation_config: MossGenerationConfig,
        _requested_chunk_ms: Option<u32>,
        mut on_pcm_chunk: F,
    ) -> Result<TtsResult, MossTtsError>
    where
        F: FnMut(Vec<f32>, bool) -> Result<(), MossTtsError>,
    {
        let (_, mut state) = self.ensure_codec_decode_step(assets)?;
        let mut pending_frames: Vec<Vec<i64>> = Vec::new();
        let mut budget = FrameBudget::new(state.batch_size, &assets.codec_meta.codec_config);
        let mut buffer = PcmChunkBuffer::default();
        let total_started = Instant::now();

        self.generate_audio_frames_with_callback(
            assets,
            request,
            sampling_mode,
            generation_config,
            |sessions, frame| {
                pending_frames.push(frame.to_vec());
                if pending_frames.len() >= budget.next_batch_size(false) {
                    let frames = std::mem::take(&mut pending_frames);
                    let decode_started = Instant::now();
                    let chunk = sessions.run_codec_decode_step_batch(assets, &frames, &mut state)?;
                    trace_moss_stage(
                        "codec_decode_step_stream_batch",
                        decode_started,
                        format_args!("frames={} pcm_samples={}", frames.len(), chunk.len()),
                    );
                    budget.record_pcm_samples(chunk.len());
                    on_pcm_chunk(chunk.clone(), false)?;
                    buffer.push_chunk(chunk);
                }
                Ok(())
            },
        )?;

        while !pending_frames.is_empty() {
            let take = budget.next_batch_size(true).min(pending_frames.len());
            let frames = pending_frames.drain(..take).collect::<Vec<_>>();
            let is_final = pending_frames.is_empty();
            let decode_started = Instant::now();
            let chunk = self.run_codec_decode_step_batch(assets, &frames, &mut state)?;
            trace_moss_stage(
                "codec_decode_step_stream_batch",
                decode_started,
                format_args!("frames={} pcm_samples={} final={is_final}", frames.len(), chunk.len()),
            );
            budget.record_pcm_samples(chunk.len());
            on_pcm_chunk(chunk.clone(), is_final)?;
            buffer.push_chunk(chunk);
        }

        let result = buffer.into_tts_result()?;
        trace_moss_stage(
            "synthesize_streaming_total",
            total_started,
            format_args!("pcm_samples={}", pcm_sample_count(&result.audio.pcm)),
        );
        self.end_ort_profiling();
        Ok(result)
    }

    fn decode_step_buffered(
        &mut self,
        assets: &MossAssets,
        audio_frames: &[Vec<i64>],
    ) -> Result<TtsResult, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(codec_decode_step_unavailable(
                "no audio frames to decode".to_string(),
            ));
        }
        let (_, mut state) = self.ensure_codec_decode_step(assets)?;
        let batch_size = state.batch_size;
        let mut buffer = PcmChunkBuffer::default();
        for batch in audio_frames.chunks(batch_size) {
            let chunk = self.run_codec_decode_step_batch(assets, batch, &mut state)?;
            buffer.push_chunk(chunk);
        }
        buffer.into_tts_result()
    }

    fn run_codec_decode_step_batch(
        &mut self,
        assets: &MossAssets,
        audio_frames: &[Vec<i64>],
        state: &mut CodecDecodeStepState,
    ) -> Result<Vec<f32>, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(codec_decode_step_unavailable(
                "decode step batch contains no frames".to_string(),
            ));
        }
        let frames = audio_frames.len();
        let quantizers = audio_frames.first().map(Vec::len).unwrap_or(0);
        let audio_codes = audio_frames
            .iter()
            .flatten()
            .map(|value| to_i32(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let codes =
            Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
                MossTtsError::Inference {
                    stage: "codec_decode_step",
                    detail: e.to_string(),
                }
            })?;
        let lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);

        let mut i32_state_tensors = Vec::new();
        let mut f32_state_tensors = Vec::new();
        state.collect_input_views(&mut i32_state_tensors, &mut f32_state_tensors)?;

        let mut inputs = ort::inputs![
            "audio_codes" => TensorRef::from_array_view(codes.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_step", detail: e.to_string() })?,
            "audio_code_lengths" => TensorRef::from_array_view(lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_step", detail: e.to_string() })?
        ];
        for (name, tensor) in &i32_state_tensors {
            inputs.push((
                name.clone().into(),
                SessionInputValue::from(
                    TensorRef::from_array_view(tensor.view()).map_err(|e| {
                        MossTtsError::Inference {
                            stage: "codec_decode_step",
                            detail: e.to_string(),
                        }
                    })?,
                ),
            ));
        }
        for (name, tensor) in &f32_state_tensors {
            inputs.push((
                name.clone().into(),
                SessionInputValue::from(
                    TensorRef::from_array_view(tensor.view()).map_err(|e| {
                        MossTtsError::Inference {
                            stage: "codec_decode_step",
                            detail: e.to_string(),
                        }
                    })?,
                ),
            ));
        }

        let outputs = self
            .ensure_codec_decode_step(assets)?
            .0
            .run(inputs)
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_step",
                detail: e.to_string(),
            })?;
        let audio = extract_codec_audio_chunk(&outputs, "codec_decode_step")?;
        let mut owned_state_outputs = collect_codec_decode_state_outputs(&outputs, state)?;
        state.update_from_owned_outputs(&mut owned_state_outputs)?;
        Ok(audio)
    }

    fn decode_full_audio(
        &mut self,
        assets: &MossAssets,
        audio_frames: &[Vec<i64>],
    ) -> Result<TtsResult, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: "no audio frames to decode".to_string(),
            });
        }
        let frames = audio_frames.len();
        let quantizers = audio_frames.first().map(Vec::len).unwrap_or(0);
        let audio_codes = audio_frames
            .iter()
            .flatten()
            .map(|value| to_i32(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let codes =
            Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
                MossTtsError::Inference {
                    stage: "codec_decode_full",
                    detail: e.to_string(),
                }
            })?;
        let lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);
        let outputs = self
            .ensure_codec_decode_full(assets)?
            .run(ort::inputs![
                "audio_codes" => TensorRef::from_array_view(codes.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?,
                "audio_code_lengths" => TensorRef::from_array_view(lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            })?;
        let samples = extract_codec_audio_chunk(&outputs, "codec_decode_full")?;
        PcmChunkBuffer::from_chunk(samples).into_tts_result()
    }

    fn generate_audio_frames(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
        generation_config: MossGenerationConfig,
    ) -> Result<Vec<Vec<i64>>, MossTtsError> {
        self.generate_audio_frames_with_callback(
            assets,
            request,
            sampling_mode,
            generation_config,
            |_sessions, _frame| Ok(()),
        )
    }

    fn generate_audio_frames_with_callback<F>(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
        generation_config: MossGenerationConfig,
        mut on_frame: F,
    ) -> Result<Vec<Vec<i64>>, MossTtsError>
    where
        F: FnMut(&mut MossSessions, &[i64]) -> Result<(), MossTtsError>,
    {
        let row_width = assets.row_width();
        let input_ids = Array3::from_shape_vec(
            (1, request.rows.len(), row_width),
            request
                .rows
                .concat()
                .into_iter()
                .map(to_i32)
                .collect::<Result<Vec<_>, _>>()?,
        )
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;
        let attention_mask = Array2::from_shape_vec(
            (1, request.attention_mask.len()),
            request
                .attention_mask
                .into_iter()
                .map(to_i32)
                .collect::<Result<Vec<_>, _>>()?,
        )
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;

        let prefill_started = Instant::now();
        let mut outputs = self
            .ensure_prefill(assets)?
            .run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(input_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?,
                "attention_mask" => TensorRef::from_array_view(attention_mask.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;
        trace_moss_stage(
            "tts_prefill",
            prefill_started,
            format_args!("rows={} row_width={row_width}", request.rows.len()),
        );

        let mut global_hidden = take_last_hidden_output(&mut outputs, "global_hidden", "tts_prefill")?;
        let mut decode_past = take_decode_present_outputs(&mut outputs, assets, "tts_prefill")?;
        let initial_past_valid_length = input_ids.shape()[1] as i64;
        drop(outputs);

        let mut frames = Vec::new();
        let n_vq = assets.n_vq();
        let codebook_size = assets.audio_codebook_size();
        let mut repetition_seen_mask = vec![0i32; n_vq * codebook_size];
        let mut rng = SimpleRng::from_optional_seed(generation_config.seed);
        let max_new_frames = generation_config.frame_limit(assets);

        for past_valid_length in (initial_past_valid_length..).take(max_new_frames) {
            let sample_started = Instant::now();
            let fixed = match sampling_mode {
                MossSamplingMode::Fixed => self.run_local_fixed_sampled_frame(
                    &global_hidden,
                    &repetition_seen_mask,
                    &mut rng,
                    assets,
                )?,
                MossSamplingMode::Greedy => self.run_local_greedy_frame(&global_hidden, assets)?,
            };
            trace_moss_stage(
                match sampling_mode {
                    MossSamplingMode::Fixed => "tts_local_fixed_sampled_frame",
                    MossSamplingMode::Greedy => "tts_local_decoder_greedy_frame",
                },
                sample_started,
                format_args!("generated_frames={}", frames.len()),
            );
            if !fixed.should_continue {
                break;
            }
            for (channel, token) in fixed.frame.iter().enumerate() {
                if let Ok(token) = usize::try_from(*token) {
                    if let Some(seen) =
                        repetition_seen_mask.get_mut(channel * codebook_size + token)
                    {
                        *seen = 1;
                    }
                }
            }
            let frame = fixed.frame;
            let decode_started = Instant::now();
            let mut decode_outputs = self.run_decode_step(&frame, past_valid_length, decode_past, assets)?;
            trace_moss_stage(
                "tts_decode_step",
                decode_started,
                format_args!("past_valid_length={past_valid_length}"),
            );
            global_hidden = take_last_hidden_output(&mut decode_outputs, "global_hidden", "tts_decode_step")?;
            decode_past = take_decode_present_outputs(&mut decode_outputs, assets, "tts_decode_step")?;
            drop(decode_outputs);
            on_frame(self, &frame)?;
            frames.push(frame);
        }

        if frames.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "tts_generate_frames",
                detail: "MOSS generated no audio frames".to_string(),
            });
        }
        Ok(frames)
    }

    fn run_local_fixed_sampled_frame(
        &mut self,
        global_hidden: &DynValue,
        repetition_seen_mask_data: &[i32],
        rng: &mut SimpleRng,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let n_vq = assets.n_vq();
        let codebook_size = assets.audio_codebook_size();
        let expected_mask_len = n_vq * codebook_size;
        if repetition_seen_mask_data.len() != expected_mask_len {
            return Err(MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: format!(
                    "expected repetition mask length {expected_mask_len}, got {}",
                    repetition_seen_mask_data.len()
                ),
            });
        }
        let repetition_seen_mask = ArrayViewD::from_shape(
            IxDyn(&[1, n_vq, codebook_size]),
            repetition_seen_mask_data,
        )
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;
        let assistant_random_u = Array1::from_vec(vec![rng.next_f32()]);
        let audio_random_u = Array2::from_shape_vec((1, n_vq), (0..n_vq).map(|_| rng.next_f32()).collect())
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;

        let outputs = self
            .ensure_local_fixed_sampled_frame(assets)?
            .run(ort::inputs![
                "global_hidden" => global_hidden,
                "repetition_seen_mask" => TensorRef::from_array_view(repetition_seen_mask.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?,
                "assistant_random_u" => TensorRef::from_array_view(assistant_random_u.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?,
                "audio_random_u" => TensorRef::from_array_view(audio_random_u.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;
        let should_continue = extract_first_i64(&outputs, "should_continue", "tts_local_fixed_sampled_frame")? > 0;
        let frame = extract_i64_tensor(&outputs, "frame_token_ids", "tts_local_fixed_sampled_frame")?;
        if frame.len() != n_vq {
            return Err(MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: format!("expected {n_vq} frame tokens, got {}", frame.len()),
            });
        }
        Ok(GeneratedFrame { should_continue, frame })
    }

    fn run_local_greedy_frame(
        &mut self,
        global_hidden: &DynValue,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let n_vq = assets.n_vq();
        let mut frame = Vec::with_capacity(n_vq);
        let text_token_id = Array1::from_vec(vec![to_i32(
            assets.manifest.tts_config.audio_assistant_slot_token_id,
        )?]);
        let empty_prefix = greedy_audio_prefix(&frame, assets)?;
        let outputs = self
            .ensure_local_decoder(assets)?
            .run(ort::inputs![
                "global_hidden" => global_hidden,
                "text_token_id" => TensorRef::from_array_view(text_token_id.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_decoder", detail: e.to_string() })?,
                "audio_prefix_token_ids" => TensorRef::from_array_view(empty_prefix.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_decoder", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_decoder",
                detail: e.to_string(),
            })?;
        let assistant_token = argmax_i64_output(&outputs, "text_logits", "tts_local_decoder")?;
        if assistant_token != assets.manifest.tts_config.audio_assistant_slot_token_id {
            return Ok(GeneratedFrame {
                should_continue: false,
                frame,
            });
        }
        let first_token = argmax_audio_logit(&outputs, "audio_logits", 0, "tts_local_decoder")?;
        drop(outputs);
        frame.push(first_token);

        for channel in 1..n_vq {
            let prefix = greedy_audio_prefix(&frame, assets)?;
            let outputs = self
                .ensure_local_decoder(assets)?
                .run(ort::inputs![
                    "global_hidden" => global_hidden,
                    "text_token_id" => TensorRef::from_array_view(text_token_id.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_decoder", detail: e.to_string() })?,
                    "audio_prefix_token_ids" => TensorRef::from_array_view(prefix.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_decoder", detail: e.to_string() })?
                ])
                .map_err(|e| MossTtsError::Inference {
                    stage: "tts_local_decoder",
                    detail: e.to_string(),
                })?;
            let token = argmax_audio_logit(
                &outputs,
                "audio_logits",
                channel,
                "tts_local_decoder",
            )?;
            drop(outputs);
            frame.push(token);
        }
        Ok(GeneratedFrame {
            should_continue: true,
            frame,
        })
    }

    fn run_decode_step<'a>(
        &'a mut self,
        frame: &[i64],
        past_valid_length: i64,
        decode_past: Vec<(String, DynValue)>,
        assets: &MossAssets,
    ) -> Result<ort::session::SessionOutputs<'a>, MossTtsError> {
        let mut row = vec![assets.manifest.tts_config.audio_pad_token_id; assets.row_width()];
        row[0] = assets.manifest.tts_config.audio_assistant_slot_token_id;
        for (index, token) in frame.iter().take(assets.n_vq()).enumerate() {
            row[index + 1] = *token;
        }
        let input_ids = Array3::from_shape_vec(
            (1, 1, assets.row_width()),
            row.into_iter().map(to_i32).collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })?;
        let past_valid_lengths = Array1::from_vec(vec![to_i32(past_valid_length)?]);
        let mut inputs = ort::inputs![
            "input_ids" => TensorRef::from_array_view(input_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_decode_step", detail: e.to_string() })?,
            "past_valid_lengths" => TensorRef::from_array_view(past_valid_lengths.view()).map_err(|e| MossTtsError::Inference { stage: "tts_decode_step", detail: e.to_string() })?
        ];
        for (name, value) in decode_past {
            inputs.push((name.into(), SessionInputValue::from(value)));
        }
        self.ensure_decode_step(assets)?.run(inputs).map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })
    }

}

fn interleave_codec_audio(
    audio_shape: &[i64],
    audio_data: &[f32],
    audio_len: usize,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    if audio_shape.len() != 3 || audio_shape[0] != 1 || audio_shape[1] != PLAYBACK_CHANNELS as i64 {
        return Err(MossTtsError::OutputFormat(format!(
            "expected codec audio shape [1, {}, samples], got {:?}",
            PLAYBACK_CHANNELS, audio_shape
        )));
    }
    let total_samples = audio_shape[2].max(0) as usize;
    let channels = PLAYBACK_CHANNELS as usize;
    let expected = total_samples * channels;
    if audio_data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("codec audio data too short: expected {expected}, got {}", audio_data.len()),
        });
    }
    let audio_len = audio_len.min(total_samples);
    let mut samples = Vec::with_capacity(audio_len * channels);
    for sample_index in 0..audio_len {
        for channel_index in 0..channels {
            samples.push(audio_data[channel_index * total_samples + sample_index]);
        }
    }
    Ok(samples)
}

fn extract_codec_audio_chunk(
    outputs: &ort::session::SessionOutputs<'_>,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    let (audio_shape, audio_data) = outputs
        .get("audio")
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "missing audio output".to_string(),
        })?
        .try_extract_tensor::<f32>()
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })?;
    let audio_len = outputs
        .get("audio_lengths")
        .or_else(|| outputs.get("audio_length"))
        .and_then(extract_audio_length_value)
        .map(|len| len.max(0) as usize)
        .unwrap_or(audio_shape[2] as usize)
        .min(audio_shape[2] as usize);
    interleave_codec_audio(audio_shape, audio_data, audio_len, stage)
}

fn extract_audio_length_value(value: &DynValue) -> Option<i64> {
    if let Ok((_, data)) = value.try_extract_tensor::<i64>() {
        return data.first().copied();
    }
    value
        .try_extract_tensor::<i32>()
        .ok()
        .and_then(|(_, data)| data.first().map(|value| *value as i64))
}

fn collect_codec_decode_state_outputs(
    outputs: &ort::session::SessionOutputs<'_>,
    state: &CodecDecodeStepState,
) -> Result<HashMap<String, OwnedTensorData>, MossTtsError> {
    let mut values = HashMap::new();
    for offset in &state.transformer_offsets {
        values.insert(
            offset.output_name.clone(),
            extract_owned_i32_tensor(outputs, &offset.output_name, "codec_decode_step")?,
        );
    }
    for cache in &state.attention_caches {
        values.insert(
            cache.offset.output_name.clone(),
            extract_owned_i32_tensor(outputs, &cache.offset.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.keys.output_name.clone(),
            extract_owned_f32_tensor(outputs, &cache.keys.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.values.output_name.clone(),
            extract_owned_f32_tensor(outputs, &cache.values.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.positions.output_name.clone(),
            extract_owned_i32_tensor(outputs, &cache.positions.output_name, "codec_decode_step")?,
        );
    }
    Ok(values)
}

fn extract_owned_i32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<OwnedTensorData, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (shape, data) = value.try_extract_tensor::<i32>().map_err(|e| {
        MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        }
    })?;
    Ok(OwnedTensorData::I32 {
        shape: shape.iter().map(|dim| (*dim).max(0) as usize).collect(),
        data: data.to_vec(),
    })
}

fn extract_owned_f32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<OwnedTensorData, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| {
        MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        }
    })?;
    Ok(OwnedTensorData::F32 {
        shape: shape.iter().map(|dim| (*dim).max(0) as usize).collect(),
        data: data.to_vec(),
    })
}

const DEFAULT_MOSS_TTS_INTRA_THREADS: usize = 4;
const DEFAULT_MOSS_TTS_INTER_THREADS: usize = 1;
const MOSS_TTS_INTRA_THREADS_ENV: &str = "MOSS_TTS_INTRA_THREADS";
const MOSS_TTS_INTER_THREADS_ENV: &str = "MOSS_TTS_INTER_THREADS";
const MOSS_TTS_PARALLEL_EXECUTION_ENV: &str = "MOSS_TTS_PARALLEL_EXECUTION";
const MOSS_TTS_MEMORY_PATTERN_ENV: &str = "MOSS_TTS_MEMORY_PATTERN";
const MOSS_TTS_ORT_PROFILE_ENV: &str = "MOSS_TTS_ORT_PROFILE";
const MOSS_TTS_ORT_PROFILE_DIR_ENV: &str = "MOSS_TTS_ORT_PROFILE_DIR";

fn create_session(path: &Path, model_name: &'static str) -> Result<Session, MossTtsError> {
    let intra_threads = moss_tts_intra_threads();
    let inter_threads = moss_tts_inter_threads();
    let parallel_execution = moss_tts_parallel_execution();
    let memory_pattern = moss_tts_memory_pattern();
    let profiling_file = moss_tts_ort_profile_file(model_name);
    Session::builder()
        .and_then(|mut b| {
            b = b
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(intra_threads)?
                .with_inter_threads(inter_threads)?
                .with_parallel_execution(parallel_execution)?
                .with_memory_pattern(memory_pattern)?;
            if let Some(profiling_file) = profiling_file {
                b = b.with_profiling(profiling_file)?;
            }
            b.commit_from_file(path)
        })
        .map_err(|e| MossTtsError::Inference {
            stage: "session_init",
            detail: format!("{model_name}: {e}"),
        })
}

fn moss_tts_intra_threads() -> usize {
    std::env::var(MOSS_TTS_INTRA_THREADS_ENV)
        .ok()
        .and_then(|value| parse_moss_tts_threads(&value))
        .unwrap_or(DEFAULT_MOSS_TTS_INTRA_THREADS)
}

fn moss_tts_inter_threads() -> usize {
    std::env::var(MOSS_TTS_INTER_THREADS_ENV)
        .ok()
        .and_then(|value| parse_moss_tts_threads(&value))
        .unwrap_or(DEFAULT_MOSS_TTS_INTER_THREADS)
}

fn parse_moss_tts_threads(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|threads| *threads > 0)
}

fn moss_tts_parallel_execution() -> bool {
    std::env::var(MOSS_TTS_PARALLEL_EXECUTION_ENV)
        .ok()
        .and_then(|value| parse_bool_env(&value))
        .unwrap_or(false)
}

fn moss_tts_memory_pattern() -> bool {
    std::env::var(MOSS_TTS_MEMORY_PATTERN_ENV)
        .ok()
        .and_then(|value| parse_bool_env(&value))
        .unwrap_or(true)
}

fn moss_tts_ort_profile_file(model_name: &'static str) -> Option<PathBuf> {
    let enabled = std::env::var(MOSS_TTS_ORT_PROFILE_ENV)
        .ok()
        .and_then(|value| parse_bool_env(&value))
        .unwrap_or(false);
    if !enabled {
        return None;
    }
    let dir = std::env::var(MOSS_TTS_ORT_PROFILE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("voice-coding-moss-ort-profiles"));
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::warn!(
            target: "tts_moss::perf",
            "failed to create MOSS ORT profile dir {}: {}",
            dir.display(),
            error
        );
        return None;
    }
    let session_name = model_name.replace('.', "_");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Some(dir.join(format!("{session_name}-{stamp}.json")))
}

fn end_session_profiling(session: &mut Session, model_name: &'static str) {
    if !std::env::var(MOSS_TTS_ORT_PROFILE_ENV)
        .ok()
        .and_then(|value| parse_bool_env(&value))
        .unwrap_or(false)
    {
        return;
    }
    match session.end_profiling() {
        Ok(path) => {
            log::info!(
                target: "tts_moss::perf",
                "moss_ort_profile session={} path={}",
                model_name,
                path
            );
            eprintln!("moss_ort_profile session={model_name} path={path}");
        }
        Err(error) => {
            log::warn!(
                target: "tts_moss::perf",
                "failed to end MOSS ORT profiling for {}: {}",
                model_name,
                error
            );
        }
    }
}

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn validate_session_io<I, O>(
    session: &Session,
    model_name: &'static str,
    expected_inputs: I,
    expected_outputs: O,
) -> Result<(), MossTtsError>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
    O: IntoIterator,
    O::Item: AsRef<str>,
{
    for name in expected_inputs {
        let name = name.as_ref();
        if !session.inputs().iter().any(|input| input.name() == name) {
            return Err(MossTtsError::MetadataMismatch(format!(
                "{model_name} missing input '{name}'"
            )));
        }
    }
    for name in expected_outputs {
        let name = name.as_ref();
        if !session.outputs().iter().any(|output| output.name() == name) {
            return Err(MossTtsError::MetadataMismatch(format!(
                "{model_name} missing output '{name}'"
            )));
        }
    }
    Ok(())
}

fn required_file<'a>(files: &'a HashMap<String, PathBuf>, key: &str) -> Result<&'a Path, MossTtsError> {
    files
        .get(key)
        .map(PathBuf::as_path)
        .ok_or_else(|| MossTtsError::MetadataMismatch(format!("missing model file key '{key}'")))
}

fn to_i32(value: i64) -> Result<i32, MossTtsError> {
    i32::try_from(value).map_err(|_| MossTtsError::Inference {
        stage: "tensor_cast",
        detail: format!("value {value} does not fit in int32 tensor input"),
    })
}

fn validate_meta_consistency(
    manifest: &Manifest,
    tts_meta: &TtsMeta,
    codec_meta: &CodecMeta,
) -> Result<(), MossTtsError> {
    if manifest.format_version == 0 || tts_meta.format_version == 0 || codec_meta.format_version == 0 {
        return Err(MossTtsError::MetadataMismatch(
            "format_version must be greater than zero".to_string(),
        ));
    }
    if manifest.tts_config.n_vq != codec_meta.codec_config.num_quantizers {
        return Err(MossTtsError::MetadataMismatch(format!(
            "manifest n_vq {} != codec num_quantizers {}",
            manifest.tts_config.n_vq, codec_meta.codec_config.num_quantizers
        )));
    }
    if codec_meta.codec_config.sample_rate != PLAYBACK_SAMPLE_RATE_HZ {
        return Err(MossTtsError::OutputFormat(format!(
            "codec sample_rate must be {}, got {}",
            PLAYBACK_SAMPLE_RATE_HZ, codec_meta.codec_config.sample_rate
        )));
    }
    if codec_meta.codec_config.channels != PLAYBACK_CHANNELS {
        return Err(MossTtsError::OutputFormat(format!(
            "codec channels must be {}, got {}",
            PLAYBACK_CHANNELS, codec_meta.codec_config.channels
        )));
    }
    Ok(())
}

fn validate_required_model_files(
    base_dir: &Path,
    prefix: &'static str,
    files: &HashMap<String, String>,
    external_data_files: &HashMap<String, Vec<String>>,
    required_keys: &[&str],
) -> Result<HashMap<String, PathBuf>, MossTtsError> {
    let mut resolved = HashMap::new();
    for key in required_keys {
        let raw_path = files
            .get(*key)
            .ok_or_else(|| MossTtsError::MetadataMismatch(format!("missing model file key '{key}'")))?;
        let path = resolve_manifest_path(base_dir, &format!("{prefix}.files.{key}"), raw_path)?;
        ensure_file(&path, "onnx")?;
        if let Some(external_files) = external_data_files.get(raw_path) {
            for external in external_files {
                let external_path = resolve_manifest_path(
                    base_dir,
                    &format!("{prefix}.external_data_files.{raw_path}"),
                    external,
                )?;
                ensure_file(&external_path, "external data")?;
            }
        }
        resolved.insert((*key).to_string(), path);
    }
    Ok(resolved)
}

fn validate_model_files(
    base_dir: &Path,
    prefix: &'static str,
    files: &HashMap<String, String>,
    external_data_files: &HashMap<String, Vec<String>>,
    required_keys: &[&str],
    optional_keys: &[&str],
) -> Result<HashMap<String, PathBuf>, MossTtsError> {
    let mut resolved = validate_required_model_files(
        base_dir,
        prefix,
        files,
        external_data_files,
        required_keys,
    )?;
    for key in optional_keys {
        let Some(raw_path) = files.get(*key) else {
            continue;
        };
        let path = resolve_manifest_path(base_dir, &format!("{prefix}.files.{key}"), raw_path)?;
        if !path.is_file() {
            continue;
        }
        if let Some(external_files) = external_data_files.get(raw_path) {
            let mut all_external_present = true;
            for external in external_files {
                let external_path = resolve_manifest_path(
                    base_dir,
                    &format!("{prefix}.external_data_files.{raw_path}"),
                    external,
                )?;
                if !external_path.is_file() {
                    all_external_present = false;
                    break;
                }
            }
            if !all_external_present {
                continue;
            }
        }
        resolved.insert((*key).to_string(), path);
    }
    Ok(resolved)
}

fn resolve_manifest_path(base_dir: &Path, field: &str, raw: &str) -> Result<PathBuf, MossTtsError> {
    let path = Path::new(raw);
    if path.is_absolute() || path.components().any(|c| matches!(c, Component::Prefix(_))) {
        return Err(MossTtsError::InvalidRelativePath {
            field: field.to_string(),
            raw: raw.to_string(),
        });
    }
    Ok(base_dir.join(path))
}

fn ensure_file(path: &Path, kind: &'static str) -> Result<(), MossTtsError> {
    if !path.is_file() {
        return Err(MossTtsError::MissingFile {
            kind,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, MossTtsError> {
    let bytes = std::fs::read(path).map_err(|e| MossTtsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|e| MossTtsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

struct GeneratedFrame {
    should_continue: bool,
    frame: Vec<i64>,
}

fn take_output(
    outputs: &mut ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<DynValue, MossTtsError> {
    outputs.remove(name).ok_or_else(|| MossTtsError::Inference {
        stage,
        detail: format!("missing output '{name}'"),
    })
}

fn take_last_hidden_output(
    outputs: &mut ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<DynValue, MossTtsError> {
    let value = take_output(outputs, name, stage)?;
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    match shape.as_ref() {
        [1, _, hidden_size] => {
            let hidden_size = *hidden_size as usize;
            let seq_len = shape[1] as usize;
            if seq_len == 0 {
                return Err(MossTtsError::Inference {
                    stage,
                    detail: format!("output '{name}' has empty sequence axis"),
                });
            }
            let hidden = Array3::from_shape_vec((1, seq_len, hidden_size), data.to_vec())
                .map_err(|e| MossTtsError::Inference {
                    stage,
                    detail: e.to_string(),
                })?;
            let last_hidden = hidden.slice(s![0..1, seq_len - 1, ..]).to_owned();
            Value::from_array(last_hidden)
                .map(|tensor| tensor.into_dyn())
                .map_err(|e| MossTtsError::Inference {
                    stage,
                    detail: e.to_string(),
                })
        }
        [1, _] => Ok(value),
        other => Err(MossTtsError::Inference {
            stage,
            detail: format!("unexpected output '{name}' shape {other:?}"),
        }),
    }
}

fn take_decode_present_outputs(
    outputs: &mut ort::session::SessionOutputs<'_>,
    assets: &MossAssets,
    stage: &'static str,
) -> Result<Vec<(String, DynValue)>, MossTtsError> {
    let (input_names, output_names) = decode_present_name_pairs(assets)?;
    let mut values = Vec::new();
    for (input_name, output_name) in input_names.into_iter().zip(output_names) {
        values.push((input_name.clone(), take_output(outputs, output_name, stage)?));
    }
    Ok(values)
}

fn decode_present_name_pairs(assets: &MossAssets) -> Result<(Vec<&String>, Vec<&String>), MossTtsError> {
    let input_names = assets
        .tts_meta
        .onnx
        .decode_input_names
        .iter()
        .skip(2)
        .collect::<Vec<_>>();
    let output_names = assets
        .tts_meta
        .onnx
        .decode_output_names
        .iter()
        .skip(1)
        .collect::<Vec<_>>();
    if input_names.len() != output_names.len() {
        return Err(MossTtsError::MetadataMismatch(format!(
            "decode past input/output count mismatch: {} inputs, {} outputs",
            input_names.len(),
            output_names.len()
        )));
    }
    Ok((input_names, output_names))
}

fn extract_i64_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<Vec<i64>, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    if let Ok((_, data)) = value.try_extract_tensor::<i64>() {
        return Ok(data.to_vec());
    }
    value
        .try_extract_tensor::<i32>()
        .map(|(_, data)| data.iter().map(|value| *value as i64).collect())
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })
}

fn extract_audio_codes(
    outputs: &ort::session::SessionOutputs<'_>,
    stage: &'static str,
) -> Result<Vec<Vec<i64>>, MossTtsError> {
    let value = outputs
        .get("audio_codes")
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "missing output 'audio_codes'".to_string(),
        })?;
    let (shape, data) = value.try_extract_tensor::<i32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    if shape.len() != 3 || shape[0] != 1 {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("expected audio_codes shape [1, frames, quantizers], got {shape:?}"),
        });
    }
    let frames = shape[1].max(0) as usize;
    let quantizers = shape[2].max(0) as usize;
    let expected = frames * quantizers;
    if data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("audio_codes data too short: expected {expected}, got {}", data.len()),
        });
    }
    let length = outputs
        .get("audio_code_lengths")
        .and_then(|value| value.try_extract_tensor::<i32>().ok())
        .and_then(|(_, data)| data.first().copied())
        .map(|value| value.max(0) as usize)
        .unwrap_or(frames)
        .min(frames);
    audio_codes_from_flat_data(shape, data, length, stage)
}

fn audio_codes_from_flat_data(
    shape: &[i64],
    data: &[i32],
    length: usize,
    stage: &'static str,
) -> Result<Vec<Vec<i64>>, MossTtsError> {
    if shape.len() != 3 || shape[0] != 1 {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("expected audio_codes shape [1, frames, quantizers], got {shape:?}"),
        });
    }
    let frames = shape[1].max(0) as usize;
    let quantizers = shape[2].max(0) as usize;
    let expected = frames * quantizers;
    if data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("audio_codes data too short: expected {expected}, got {}", data.len()),
        });
    }
    let length = length.min(frames);
    let mut codes = Vec::with_capacity(length);
    for frame_index in 0..length {
        let start = frame_index * quantizers;
        codes.push(
            data[start..start + quantizers]
                .iter()
                .map(|value| *value as i64)
                .collect(),
        );
    }
    if codes.is_empty() {
        return Err(MossTtsError::Inference {
            stage,
            detail: "codec encode produced no audio codes".to_string(),
        });
    }
    Ok(codes)
}

fn extract_first_i64(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<i64, MossTtsError> {
    extract_i64_tensor(outputs, name, stage)?
        .first()
        .copied()
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("output '{name}' is empty"),
        })
}

fn argmax_i64_output(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<i64, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (_, data) = value.try_extract_tensor::<f32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    data.iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index as i64)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("output '{name}' is empty"),
        })
}

fn argmax_audio_logit(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    channel: usize,
    stage: &'static str,
) -> Result<i64, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    let (channels, vocab) = match shape.as_ref() {
        [vocab] => (1usize, *vocab as usize),
        [1, vocab] => (1usize, *vocab as usize),
        [channels, vocab] => (*channels as usize, *vocab as usize),
        [1, channels, vocab] => (*channels as usize, *vocab as usize),
        other => {
            return Err(MossTtsError::Inference {
                stage,
                detail: format!("unexpected output '{name}' shape {other:?}"),
            })
        }
    };
    if vocab == 0 || data.is_empty() {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("output '{name}' is empty"),
        });
    }
    let channel_index = if channels == 1 { 0 } else { channel };
    let start = channel_index.checked_mul(vocab).ok_or_else(|| MossTtsError::Inference {
        stage,
        detail: "audio logits offset overflowed".to_string(),
    })?;
    let end = start.checked_add(vocab).ok_or_else(|| MossTtsError::Inference {
        stage,
        detail: "audio logits range overflowed".to_string(),
    })?;
    let logits = data.get(start..end).ok_or_else(|| MossTtsError::Inference {
        stage,
        detail: format!(
            "output '{name}' does not contain channel {channel} logits for shape {shape:?}"
        ),
    })?;
    logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index as i64)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("output '{name}' channel {channel} is empty"),
        })
}

fn greedy_audio_prefix(
    frame: &[i64],
    assets: &MossAssets,
) -> Result<Array2<i32>, MossTtsError> {
    let prefix_len = assets.n_vq().saturating_sub(1);
    let mut prefix = vec![to_i32(assets.manifest.tts_config.audio_pad_token_id)?; prefix_len];
    for (index, token) in frame.iter().take(prefix_len).enumerate() {
        prefix[index] = to_i32(*token)?;
    }
    Array2::from_shape_vec((1, prefix_len), prefix).map_err(|e| MossTtsError::Inference {
        stage: "tts_local_decoder",
        detail: e.to_string(),
    })
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        let state = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self { state }
    }

    fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    fn from_optional_seed(seed: Option<u64>) -> Self {
        seed.map(Self::from_seed).unwrap_or_else(Self::new)
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        let value = ((self.state >> 40) as f32) / ((1u64 << 24) as f32);
        value.clamp(0.0, 0.999_999_94)
    }
}

fn pcm_sample_count(pcm: &PcmData) -> usize {
    match pcm {
        PcmData::I16(samples) => samples.len(),
        PcmData::F32(samples) => samples.len(),
    }
}

fn trace_moss_stage(stage: &str, started: Instant, detail: std::fmt::Arguments<'_>) {
    let elapsed_ms = started.elapsed().as_millis();
    let detail = detail.to_string();
    log::info!(
        target: "tts_moss::perf",
        "moss_tts_trace stage={} elapsed_ms={} {}",
        stage,
        elapsed_ms,
        detail,
    );
    if std::env::var("MOSS_TTS_TRACE").is_ok_and(|value| parse_bool_env(&value).unwrap_or(false)) {
        eprintln!("moss_tts_trace stage={stage} elapsed_ms={elapsed_ms} {detail}");
    }
}
