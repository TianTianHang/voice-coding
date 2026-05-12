struct MossSessions {
    #[allow(dead_code)]
    prefill: Session,
    #[allow(dead_code)]
    decode_step: Session,
    #[allow(dead_code)]
    local_decoder: Session,
    #[allow(dead_code)]
    local_cached_step: Session,
    #[allow(dead_code)]
    local_fixed_sampled_frame: Session,
    codec_encode: Session,
    codec_decode_full: Session,
    #[allow(dead_code)]
    codec_decode_step: Session,
    codec_decode_step_state: CodecDecodeStepState,
}

impl MossSessions {
    fn load(assets: &MossAssets) -> Result<Self, MossTtsError> {
        let prefill = create_session(required_file(&assets.tts_files, "prefill")?, "tts.prefill")?;
        let decode_step = create_session(required_file(&assets.tts_files, "decode_step")?, "tts.decode_step")?;
        let local_decoder = create_session(required_file(&assets.tts_files, "local_decoder")?, "tts.local_decoder")?;
        let local_cached_step = create_session(required_file(&assets.tts_files, "local_cached_step")?, "tts.local_cached_step")?;
        let local_fixed_sampled_frame = create_session(
            required_file(&assets.tts_files, "local_fixed_sampled_frame")?,
            "tts.local_fixed_sampled_frame",
        )?;
        let codec_decode_full = create_session(
            required_file(&assets.codec_files, "decode_full")?,
            "codec.decode_full",
        )?;
        let codec_encode = create_session(
            required_file(&assets.codec_files, "encode")?,
            "codec.encode",
        )?;
        let codec_decode_step = create_session(
            required_file(&assets.codec_files, "decode_step")?,
            "codec.decode_step",
        )?;

        validate_session_io(
            &prefill,
            "tts.prefill",
            ["input_ids", "attention_mask"],
            assets.tts_meta.onnx.prefill_output_names.iter(),
        )?;
        validate_session_io(
            &decode_step,
            "tts.decode_step",
            assets.tts_meta.onnx.decode_input_names.iter(),
            assets.tts_meta.onnx.decode_output_names.iter(),
        )?;
        validate_session_io(
            &local_decoder,
            "tts.local_decoder",
            &["global_hidden", "text_token_id", "audio_prefix_token_ids"],
            &["text_logits", "audio_logits"],
        )?;
        validate_session_io(
            &local_cached_step,
            "tts.local_cached_step",
            assets.tts_meta.onnx.local_cached_input_names.iter(),
            assets.tts_meta.onnx.local_cached_output_names.iter(),
        )?;
        validate_session_io(
            &local_fixed_sampled_frame,
            "tts.local_fixed_sampled_frame",
            &[
                "global_hidden",
                "repetition_seen_mask",
                "assistant_random_u",
                "audio_random_u",
            ],
            &["should_continue", "frame_token_ids"],
        )?;
        validate_session_io(
            &codec_encode,
            "codec.encode",
            assets.codec_meta.onnx.encode_input_names.iter(),
            assets.codec_meta.onnx.encode_output_names.iter(),
        )?;
        validate_session_io(
            &codec_decode_full,
            "codec.decode_full",
            assets.codec_meta.onnx.decode_input_names.iter(),
            assets.codec_meta.onnx.decode_output_names.iter(),
        )?;
        validate_session_io(
            &codec_decode_step,
            "codec.decode_step",
            assets.codec_meta.onnx.decode_step_input_names.iter(),
            assets.codec_meta.onnx.decode_step_output_names.iter(),
        )?;
        let codec_decode_step_state = CodecDecodeStepState::from_meta(&assets.codec_meta)?;

        Ok(Self {
            prefill,
            decode_step,
            local_decoder,
            local_cached_step,
            local_fixed_sampled_frame,
            codec_encode,
            codec_decode_full,
            codec_decode_step,
            codec_decode_step_state,
        })
    }

    fn encode_reference_audio(
        &mut self,
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
            .codec_encode
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
        let generated_frames =
            self.generate_audio_frames(assets, request, sampling_mode, generation_config)?;
        self.decode_generated_frames(generated_frames)
    }

    fn synthesize_streaming_frames<F>(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
        generation_config: MossGenerationConfig,
        requested_chunk_ms: Option<u32>,
        mut on_pcm_chunk: F,
    ) -> Result<TtsResult, MossTtsError>
    where
        F: FnMut(Vec<f32>, bool) -> Result<(), MossTtsError>,
    {
        let mut state = self.codec_decode_step_state.clone();
        let mut pending_frames: Vec<Vec<i64>> = Vec::new();
        let mut budget = FrameBudget::new(state.batch_size, requested_chunk_ms);
        let mut buffer = PcmChunkBuffer::default();

        self.generate_audio_frames_with_callback(
            assets,
            request,
            sampling_mode,
            generation_config,
            |sessions, frame| {
                pending_frames.push(frame.to_vec());
                if pending_frames.len() >= budget.next_batch_size(false) {
                    let frames = std::mem::take(&mut pending_frames);
                    let chunk = sessions.run_codec_decode_step_batch(&frames, &mut state)?;
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
            let chunk = self.run_codec_decode_step_batch(&frames, &mut state)?;
            budget.record_pcm_samples(chunk.len());
            on_pcm_chunk(chunk.clone(), is_final)?;
            buffer.push_chunk(chunk);
        }

        buffer.into_tts_result()
    }

    fn decode_generated_frames(
        &mut self,
        generated_frames: Vec<Vec<i64>>,
    ) -> Result<TtsResult, MossTtsError> {
        if USE_CODEC_DECODE_STEP_BY_DEFAULT {
            match self.decode_step_buffered(&generated_frames) {
                Ok(result) => Ok(result),
                Err(_) => self.decode_full(generated_frames),
            }
        } else {
            self.decode_full(generated_frames)
        }
    }

    fn decode_step_buffered(
        &mut self,
        audio_frames: &[Vec<i64>],
    ) -> Result<TtsResult, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(codec_decode_step_unavailable(
                "no audio frames to decode".to_string(),
            ));
        }
        let mut state = self.codec_decode_step_state.clone();
        let batch_size = state.batch_size;
        let mut buffer = PcmChunkBuffer::default();
        for batch in audio_frames.chunks(batch_size) {
            let chunk = self.run_codec_decode_step_batch(batch, &mut state)?;
            buffer.push_chunk(chunk);
        }
        buffer.into_tts_result()
    }

    fn run_codec_decode_step_batch(
        &mut self,
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
        state.collect_input_arrays(&mut i32_state_tensors, &mut f32_state_tensors)?;

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
            .codec_decode_step
            .run(inputs)
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_step",
                detail: e.to_string(),
            })?;
        let audio = extract_codec_audio_chunk(&outputs, "codec_decode_step")?;
        let owned_state_outputs = collect_codec_decode_state_outputs(&outputs, state)?;
        state.update_from_owned_outputs(&owned_state_outputs)?;
        Ok(audio)
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

        let mut outputs = self
            .prefill
            .run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(input_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?,
                "attention_mask" => TensorRef::from_array_view(attention_mask.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;

        let mut global_hidden = take_last_hidden_output(&mut outputs, "global_hidden", "tts_prefill")?;
        let mut decode_past = take_decode_present_outputs(&mut outputs, assets, "tts_prefill")?;
        let initial_past_valid_length = input_ids.shape()[1] as i64;
        drop(outputs);

        let mut frames = Vec::new();
        let mut previous_token_sets = vec![vec![false; assets.audio_codebook_size()]; assets.n_vq()];
        let mut rng = SimpleRng::from_optional_seed(generation_config.seed);
        let max_new_frames = generation_config.frame_limit(assets);

        for past_valid_length in (initial_past_valid_length..).take(max_new_frames) {
            let fixed = match sampling_mode {
                MossSamplingMode::Fixed => {
                    self.run_local_fixed_sampled_frame(
                        &global_hidden,
                        &previous_token_sets,
                        &mut rng,
                        assets,
                    )?
                }
                MossSamplingMode::Greedy => {
                    self.run_local_cached_sampled_frame(
                        &global_hidden,
                        &previous_token_sets,
                        &mut rng,
                        assets,
                    )?
                }
            };
            if !fixed.should_continue {
                break;
            }
            for (channel, token) in fixed.frame.iter().enumerate() {
                if let Some(seen) = previous_token_sets
                    .get_mut(channel)
                    .and_then(|row| row.get_mut(*token as usize))
                {
                    *seen = true;
                }
            }
            let frame = fixed.frame;
            let mut decode_outputs = self.run_decode_step(&frame, past_valid_length, decode_past, assets)?;
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
        previous_token_sets: &[Vec<bool>],
        rng: &mut SimpleRng,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let n_vq = assets.n_vq();
        let codebook_size = assets.audio_codebook_size();
        let mut seen = vec![0i32; n_vq * codebook_size];
        for (channel, tokens) in previous_token_sets.iter().enumerate() {
            for (token, token_seen) in tokens.iter().enumerate() {
                if *token_seen {
                    seen[channel * codebook_size + token] = 1;
                }
            }
        }
        let repetition_seen_mask = Array3::from_shape_vec((1, n_vq, codebook_size), seen)
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
            .local_fixed_sampled_frame
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

    fn run_local_cached_sampled_frame(
        &mut self,
        global_hidden: &DynValue,
        previous_token_sets: &[Vec<bool>],
        rng: &mut SimpleRng,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let assistant_slot = assets.manifest.tts_config.audio_assistant_slot_token_id;
        let defaults = &assets.manifest.generation_defaults;
        let first = self.run_local_cached_step(
            global_hidden,
            LocalCachedStepRequest {
                text_token_id: 0,
                audio_token_id: 0,
                channel_index: 0,
                step_type: LocalStepType::Text,
                local_past: None,
                past_valid_length: 0,
            },
            assets,
        )?;
        let text_token = sample_token_from_logits(
            &first.text_logits,
            defaults.text_temperature,
            defaults.text_top_p,
            defaults.text_top_k,
            rng,
            "tts_local_cached_step",
        )?;
        if text_token != assistant_slot {
            return Ok(GeneratedFrame {
                should_continue: false,
                frame: Vec::new(),
            });
        }

        let mut frame = Vec::with_capacity(assets.n_vq());
        let first_audio_step = self.run_local_cached_step(
            global_hidden,
            LocalCachedStepRequest {
                text_token_id: text_token,
                audio_token_id: 0,
                channel_index: 0,
                step_type: LocalStepType::FirstAudio,
                local_past: Some(first.local_present),
                past_valid_length: 1,
            },
            assets,
        )?;
        let first_channel_logits = channel_logits(&first_audio_step.audio_logits, 0, assets)?;
        let first_audio_logits = apply_repetition_penalty(
            first_channel_logits,
            previous_token_sets.first().map(Vec::as_slice),
            defaults.audio_repetition_penalty,
        );
        let first_audio_token = sample_token_from_logits(
            &first_audio_logits,
            defaults.audio_temperature,
            defaults.audio_top_p,
            defaults.audio_top_k,
            rng,
            "tts_local_cached_step",
        )?;
        frame.push(first_audio_token);
        let mut local_past = first_audio_step.local_present;
        for (local_past_valid_length, channel_index) in (2..).zip(1..assets.n_vq()) {
            let previous_audio_token = *frame.last().ok_or_else(|| MossTtsError::Inference {
                stage: "tts_local_greedy",
                detail: "missing previous audio token".to_string(),
            })?;
            let step = self.run_local_cached_step(
                global_hidden,
                LocalCachedStepRequest {
                    text_token_id: 0,
                    audio_token_id: previous_audio_token,
                    channel_index: channel_index - 1,
                    step_type: LocalStepType::Audio,
                    local_past: Some(local_past),
                    past_valid_length: local_past_valid_length,
                },
                assets,
            )?;
            let channel_logits = channel_logits(&step.audio_logits, channel_index, assets)?;
            let audio_logits = apply_repetition_penalty(
                channel_logits,
                previous_token_sets.get(channel_index).map(Vec::as_slice),
                defaults.audio_repetition_penalty,
            );
            frame.push(sample_token_from_logits(
                &audio_logits,
                defaults.audio_temperature,
                defaults.audio_top_p,
                defaults.audio_top_k,
                rng,
                "tts_local_cached_step",
            )?);
            local_past = step.local_present;
        }
        Ok(GeneratedFrame {
            should_continue: true,
            frame,
        })
    }

    fn run_local_cached_step(
        &mut self,
        global_hidden: &DynValue,
        request: LocalCachedStepRequest,
        assets: &MossAssets,
    ) -> Result<LocalCachedStepOutput, MossTtsError> {
        let text_token_id = Array1::from_vec(vec![to_i32(request.text_token_id)?]);
        let audio_token_id = Array1::from_vec(vec![to_i32(request.audio_token_id)?]);
        let channel_index = Array1::from_vec(vec![to_i32(request.channel_index as i64)?]);
        let step_type = Array1::from_vec(vec![request.step_type.as_i32()]);
        let past_valid_lengths = Array1::from_vec(vec![to_i32(request.past_valid_length)?]);
        let local_past = match request.local_past {
            Some(local_past) => local_past,
            None => create_empty_local_past(assets)?,
        };

        let mut inputs = ort::inputs![
            "global_hidden" => global_hidden,
            "text_token_id" => TensorRef::from_array_view(text_token_id.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_cached_step", detail: e.to_string() })?,
            "audio_token_id" => TensorRef::from_array_view(audio_token_id.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_cached_step", detail: e.to_string() })?,
            "channel_index" => TensorRef::from_array_view(channel_index.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_cached_step", detail: e.to_string() })?,
            "step_type" => TensorRef::from_array_view(step_type.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_cached_step", detail: e.to_string() })?,
            "past_valid_lengths" => TensorRef::from_array_view(past_valid_lengths.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_cached_step", detail: e.to_string() })?
        ];
        for (name, value) in local_past {
            inputs.push((name.into(), SessionInputValue::from(value)));
        }
        let mut outputs = self.local_cached_step.run(inputs).map_err(|e| MossTtsError::Inference {
            stage: "tts_local_cached_step",
            detail: e.to_string(),
        })?;
        let text_logits = extract_f32_tensor(&outputs, "text_logits", "tts_local_cached_step")?;
        let audio_logits = extract_f32_tensor(&outputs, "audio_logits", "tts_local_cached_step")?;
        let local_present = take_local_present_outputs(&mut outputs, assets, "tts_local_cached_step")?;
        Ok(LocalCachedStepOutput {
            text_logits,
            audio_logits,
            local_present,
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
        self.decode_step.run(inputs).map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })
    }

    #[allow(dead_code)]
    fn decode_full(&mut self, audio_frames: Vec<Vec<i64>>) -> Result<TtsResult, MossTtsError> {
        let frames = audio_frames.len();
        let quantizers = audio_frames.first().map(Vec::len).unwrap_or(0);
        let audio_codes = audio_frames
            .into_iter()
            .flatten()
            .map(to_i32)
            .collect::<Result<Vec<_>, _>>()?;
        let codes = Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
            MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            }
        })?;
        let lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);
        let lengths = lengths.into_shape_clone((1,)).map_err(|e| {
            MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            }
        })?;

        let outputs = self
            .codec_decode_full
            .run(ort::inputs![
                "audio_codes" => TensorRef::from_array_view(codes.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?,
                "audio_code_lengths" => TensorRef::from_array_view(lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            })?;
        let (audio_shape, audio_data) = outputs
            .get("audio")
            .ok_or_else(|| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: "missing audio output".to_string(),
            })?
            .try_extract_tensor::<f32>()
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            })?;
        let audio_len = outputs
            .get("audio_lengths")
            .or_else(|| outputs.get("audio_length"))
            .and_then(extract_audio_length_value)
            .map(|len| len.max(0) as usize)
            .unwrap_or(audio_shape[2] as usize)
            .min(audio_shape[2] as usize);
        let samples =
            interleave_codec_audio(audio_shape, audio_data, audio_len, "codec_decode_full")?;
        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(samples),
            },
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
const MOSS_TTS_INTRA_THREADS_ENV: &str = "MOSS_TTS_INTRA_THREADS";
const MOSS_TTS_PARALLEL_EXECUTION_ENV: &str = "MOSS_TTS_PARALLEL_EXECUTION";
const MOSS_TTS_MEMORY_PATTERN_ENV: &str = "MOSS_TTS_MEMORY_PATTERN";

fn create_session(path: &Path, model_name: &'static str) -> Result<Session, MossTtsError> {
    let intra_threads = moss_tts_intra_threads();
    let parallel_execution = moss_tts_parallel_execution();
    let memory_pattern = moss_tts_memory_pattern();
    Session::builder()
        .and_then(|b| {
            b.with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(intra_threads)?
                .with_inter_threads(1)?
                .with_parallel_execution(parallel_execution)?
                .with_memory_pattern(memory_pattern)?
                .commit_from_file(path)
        })
        .map_err(|e| MossTtsError::Inference {
            stage: "session_init",
            detail: format!("{model_name}: {e}"),
        })
}

fn moss_tts_intra_threads() -> usize {
    std::env::var(MOSS_TTS_INTRA_THREADS_ENV)
        .ok()
        .and_then(|value| parse_moss_tts_intra_threads(&value))
        .unwrap_or(DEFAULT_MOSS_TTS_INTRA_THREADS)
}

fn parse_moss_tts_intra_threads(value: &str) -> Option<usize> {
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

fn validate_model_files(
    base_dir: &Path,
    prefix: &'static str,
    files: &HashMap<String, String>,
    external_data_files: &HashMap<String, Vec<String>>,
) -> Result<HashMap<String, PathBuf>, MossTtsError> {
    let mut resolved = HashMap::new();
    for (key, raw_path) in files {
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
        resolved.insert(key.clone(), path);
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

struct LocalCachedStepOutput {
    text_logits: Vec<f32>,
    audio_logits: Vec<f32>,
    local_present: Vec<(String, DynValue)>,
}

struct LocalCachedStepRequest {
    text_token_id: i64,
    audio_token_id: i64,
    channel_index: usize,
    step_type: LocalStepType,
    local_past: Option<Vec<(String, DynValue)>>,
    past_valid_length: i64,
}

#[derive(Clone, Copy)]
enum LocalStepType {
    Text,
    FirstAudio,
    Audio,
}

impl LocalStepType {
    fn as_i32(self) -> i32 {
        match self {
            Self::Text => 0,
            Self::FirstAudio => 1,
            Self::Audio => 2,
        }
    }
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

fn take_local_present_outputs(
    outputs: &mut ort::session::SessionOutputs<'_>,
    assets: &MossAssets,
    stage: &'static str,
) -> Result<Vec<(String, DynValue)>, MossTtsError> {
    let input_names = assets
        .tts_meta
        .onnx
        .local_cached_input_names
        .iter()
        .skip(6)
        .collect::<Vec<_>>();
    let output_names = assets
        .tts_meta
        .onnx
        .local_cached_output_names
        .iter()
        .skip(2)
        .collect::<Vec<_>>();
    if input_names.len() != output_names.len() {
        return Err(MossTtsError::MetadataMismatch(format!(
            "local cached past input/output count mismatch: {} inputs, {} outputs",
            input_names.len(),
            output_names.len()
        )));
    }
    let mut values = Vec::new();
    for (input_name, output_name) in input_names.into_iter().zip(output_names) {
        values.push((input_name.clone(), take_output(outputs, output_name, stage)?));
    }
    Ok(values)
}

fn create_empty_local_past(assets: &MossAssets) -> Result<Vec<(String, DynValue)>, MossTtsError> {
    let input_names = assets
        .tts_meta
        .onnx
        .local_cached_input_names
        .iter()
        .skip(6)
        .collect::<Vec<_>>();
    if input_names.len() % 2 != 0 {
        return Err(MossTtsError::MetadataMismatch(format!(
            "local cached past input count must be even, got {}",
            input_names.len()
        )));
    }
    let config = &assets.tts_meta.model_config;
    if config.local_layers == 0 || config.local_heads == 0 || config.local_head_dim == 0 {
        return Err(MossTtsError::MetadataMismatch(
            "tts model_config local_layers/local_heads/local_head_dim are required for local_cached_step"
                .to_string(),
        ));
    }
    let mut past = Vec::with_capacity(input_names.len());
    for (index, input_name) in input_names.into_iter().enumerate() {
        let value = ArrayD::<f32>::zeros(IxDyn(&[
            1,
            0,
            config.local_heads,
            config.local_head_dim,
        ]));
        past.push((
            input_name.clone(),
            Value::from_array(value)
                .map(|tensor| tensor.into_dyn())
                .map_err(|e| MossTtsError::Inference {
                    stage: "tts_local_cached_step",
                    detail: format!("failed to create empty local past tensor {index}: {e}"),
                })?,
        ));
    }
    Ok(past)
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

fn extract_f32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?
        .try_extract_tensor::<f32>()
        .map(|(_, data)| data.to_vec())
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })
}

fn greedy_token_from_logits(logits: &[f32], stage: &'static str) -> Result<i64, MossTtsError> {
    let (token, _) = logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "empty logits".to_string(),
        })?;
    Ok(token as i64)
}

fn sample_token_from_logits(
    logits: &[f32],
    temperature: f32,
    top_p: f32,
    top_k: usize,
    rng: &mut SimpleRng,
    stage: &'static str,
) -> Result<i64, MossTtsError> {
    if logits.is_empty() {
        return Err(MossTtsError::Inference {
            stage,
            detail: "empty logits".to_string(),
        });
    }
    if temperature <= 0.0 {
        return greedy_token_from_logits(logits, stage);
    }
    let mut ranked = logits
        .iter()
        .enumerate()
        .map(|(index, logit)| (index, *logit / temperature))
        .collect::<Vec<_>>();
    ranked.sort_by(|(_, left), (_, right)| right.total_cmp(left));
    if top_k > 0 && ranked.len() > top_k {
        ranked.truncate(top_k);
    }

    let max_logit = ranked
        .first()
        .map(|(_, logit)| *logit)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "empty ranked logits".to_string(),
        })?;
    let mut probs = ranked
        .into_iter()
        .map(|(index, logit)| (index, (logit - max_logit).exp()))
        .collect::<Vec<_>>();
    let total = probs.iter().map(|(_, prob)| *prob).sum::<f32>();
    if !total.is_finite() || total <= 0.0 {
        return greedy_token_from_logits(logits, stage);
    }
    for (_, prob) in &mut probs {
        *prob /= total;
    }
    if top_p > 0.0 && top_p < 1.0 {
        let mut cumulative = 0.0;
        let mut keep = 0usize;
        for (_, prob) in &probs {
            cumulative += *prob;
            keep += 1;
            if cumulative >= top_p {
                break;
            }
        }
        probs.truncate(keep.max(1));
    }
    let total = probs.iter().map(|(_, prob)| *prob).sum::<f32>();
    let mut threshold = rng.next_f32() * total;
    for (index, prob) in probs {
        if threshold <= prob {
            return Ok(index as i64);
        }
        threshold -= prob;
    }
    greedy_token_from_logits(logits, stage)
}

fn apply_repetition_penalty(
    logits: &[f32],
    seen_tokens: Option<&[bool]>,
    penalty: f32,
) -> Vec<f32> {
    let Some(seen_tokens) = seen_tokens else {
        return logits.to_vec();
    };
    if penalty <= 0.0 || (penalty - 1.0).abs() < f32::EPSILON {
        return logits.to_vec();
    }
    logits
        .iter()
        .enumerate()
        .map(|(index, logit)| {
            if seen_tokens.get(index).copied().unwrap_or(false) {
                if *logit < 0.0 {
                    *logit * penalty
                } else {
                    *logit / penalty
                }
            } else {
                *logit
            }
        })
        .collect()
}

fn channel_logits<'a>(
    audio_logits: &'a [f32],
    channel_index: usize,
    assets: &MossAssets,
) -> Result<&'a [f32], MossTtsError> {
    let codebook_size = assets.audio_codebook_size();
    let expected = assets.n_vq() * codebook_size;
    if audio_logits.len() != expected {
        return Err(MossTtsError::Inference {
            stage: "tts_local_greedy",
            detail: format!("expected {expected} audio logits, got {}", audio_logits.len()),
        });
    }
    let start = channel_index.checked_mul(codebook_size).ok_or_else(|| MossTtsError::Inference {
        stage: "tts_local_greedy",
        detail: "channel index overflow".to_string(),
    })?;
    let end = start + codebook_size;
    audio_logits.get(start..end).ok_or_else(|| MossTtsError::Inference {
        stage: "tts_local_greedy",
        detail: format!("channel {channel_index} logits out of range"),
    })
}

#[allow(dead_code)]
fn greedy_frame_from_logits(
    logits: &[f32],
    n_vq: usize,
    codebook_size: usize,
) -> Result<Vec<i64>, MossTtsError> {
    let expected = n_vq * codebook_size;
    if logits.len() != expected {
        return Err(MossTtsError::Inference {
            stage: "tts_local_greedy",
            detail: format!("expected {expected} logits, got {}", logits.len()),
        });
    }
    let mut frame = Vec::with_capacity(n_vq);
    for channel_logits in logits.chunks_exact(codebook_size) {
        let (token, _) = channel_logits
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            .ok_or_else(|| MossTtsError::Inference {
                stage: "tts_local_greedy",
                detail: "empty codebook logits".to_string(),
            })?;
        frame.push(token as i64);
    }
    Ok(frame)
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
