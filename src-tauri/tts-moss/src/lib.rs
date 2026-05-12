use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};

use ndarray::{s, Array1, Array2, Array3, ArrayD, IxDyn};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::session::SessionInputValue;
use ort::value::{DynValue, TensorRef, Value};
use sentencepiece::SentencePieceProcessor;
use serde::Deserialize;
use tts_core::{
    AudioBuffer, PcmData, StreamingTextChunk, StreamingTts, StreamingTtsSession, TtsAudioChunk,
    TtsConfig, TtsEngine, TtsError, TtsResult, TtsSynthesisEvent, TtsSynthesisProgress,
    TtsSynthesisStarted, TtsTextBoundary, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ,
};

mod robust;
mod text;

use text::{MossTextPreprocessor, PreparedTextChunk};

mod reference_audio;

use reference_audio::{reference_audio_path, ReferenceAudio};

include!("config.rs");
include!("error.rs");
include!("sampling.rs");
include!("assets.rs");
include!("engine.rs");
include!("codec_buffer.rs");
include!("streaming.rs");
include!("sessions.rs");
include!("metadata.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tts_core::MossTtsConfig;

    #[test]
    fn loads_valid_asset_layout() {
        let fixture = MossFixture::new();

        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir.clone(),
        })
        .expect("assets should load");

        assert!(assets.manifest_path.ends_with(MANIFEST_FILE));
        assert!(assets.tts_files.contains_key("prefill"));
        assert!(assets.codec_files.contains_key("decode_full"));
    }

    #[test]
    fn reports_missing_required_file() {
        let fixture = MossFixture::new();
        std::fs::remove_file(fixture.tts_dir.join("tokenizer.model")).unwrap();

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("tokenizer is required");

        assert!(matches!(err, MossTtsError::MissingFile { .. }));
        assert!(err.to_string().contains("tokenizer.model"));
    }

    #[test]
    fn rejects_absolute_manifest_relative_path() {
        let fixture = MossFixture::new();
        fixture.write_manifest(
            "/tmp/tts_meta.json",
            "../codec/codec_browser_onnx_meta.json",
            "tokenizer.model",
        );

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("absolute path should fail");

        assert!(matches!(err, MossTtsError::InvalidRelativePath { .. }));
        assert!(err.to_string().contains("model_files.tts_meta"));
    }

    #[test]
    fn rejects_codec_contract_mismatch() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta(44_100, 1, 16);

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("codec contract mismatch should fail");

        assert!(matches!(err, MossTtsError::OutputFormat(_)));
        assert!(err.to_string().contains("48000"));
    }

    #[test]
    fn maps_default_and_named_voice() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        assert_eq!(assets.resolve_voice(None).unwrap().voice, DEFAULT_VOICE);
        assert_eq!(assets.resolve_voice(Some("ava")).unwrap().voice, "Ava");
    }

    #[test]
    fn rejects_unknown_voice_with_available_list() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = assets
            .resolve_voice(Some("Nobody"))
            .expect_err("unknown voice should fail");

        assert!(matches!(err, MossTtsError::UnknownVoice { .. }));
        assert!(err.to_string().contains("Junhao"));
    }

    #[test]
    fn sampling_mode_defaults_to_fixed() {
        assert_eq!(
            MossSamplingMode::from_config(&TtsConfig::default()).unwrap(),
            MossSamplingMode::Fixed
        );
    }

    #[test]
    fn sampling_mode_accepts_fixed_and_greedy() {
        let fixed = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("fixed".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };
        let greedy = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("Greedy".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };

        assert_eq!(
            MossSamplingMode::from_config(&fixed).unwrap(),
            MossSamplingMode::Fixed
        );
        assert_eq!(
            MossSamplingMode::from_config(&greedy).unwrap(),
            MossSamplingMode::Greedy
        );
    }

    #[test]
    fn sampling_mode_rejects_unknown_mode_with_available_list() {
        let config = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("creative".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };

        let err = MossSamplingMode::from_config(&config).unwrap_err();

        assert!(matches!(err, MossTtsError::UnknownSamplingMode { .. }));
        assert!(err.to_string().contains("creative"));
        assert!(err.to_string().contains("fixed"));
        assert!(err.to_string().contains("greedy"));
    }

    #[test]
    fn greedy_frame_selects_deterministic_argmax_per_codebook() {
        let logits = vec![0.1, 0.7, 0.2, 4.0, -1.0, 3.0];

        let first = greedy_frame_from_logits(&logits, 2, 3).unwrap();
        let second = greedy_frame_from_logits(&logits, 2, 3).unwrap();

        assert_eq!(first, vec![1, 0]);
        assert_eq!(first, second);
    }

    #[test]
    fn greedy_frame_rejects_unexpected_logits_shape() {
        let err = greedy_frame_from_logits(&[0.0, 1.0], 2, 3).unwrap_err();

        assert!(err.to_string().contains("expected 6 logits"));
    }

    #[test]
    fn enforces_output_contract() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let engine = MossOnnxTtsEngine::from_assets_for_test(assets);

        let result = TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: 44_100,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(vec![0.0; 2]),
            },
        };

        let err = engine
            .validate_output_contract(&result)
            .expect_err("sample rate must match playback contract");
        assert!(matches!(err, MossTtsError::OutputFormat(_)));
    }

    #[test]
    fn builds_prompt_rows_from_prepared_chunk_tokens() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let voice = assets.resolve_voice(None).unwrap();

        let request = assets
            .build_voice_clone_request_rows([101, 102, 103], &voice.prompt_audio_codes)
            .unwrap();

        assert_eq!(request.attention_mask, vec![1; request.rows.len()]);
        assert!(request.rows.iter().any(|row| row.first() == Some(&101)));
        assert!(request
            .rows
            .iter()
            .any(|row| row.first() == Some(&assets.manifest.tts_config.audio_start_token_id)));
    }

    #[test]
    fn reference_prompt_codes_replace_builtin_voice_codes() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let voice = assets.resolve_voice(None).unwrap();
        let reference_codes = vec![vec![99; assets.n_vq()]];

        let request = assets
            .build_voice_clone_request_rows([101], &reference_codes)
            .unwrap();

        assert!(request.rows.iter().any(|row| row[1] == 99));
        assert!(!request
            .rows
            .iter()
            .any(|row| row.get(1) == voice.prompt_audio_codes[0].first()));
    }

    #[test]
    fn codec_encode_codes_preserve_shape_and_length() {
        let codes =
            audio_codes_from_flat_data(&[1, 2, 3], &[1, 2, 3, 4, 5, 6], 1, "codec_encode").unwrap();

        assert_eq!(codes, vec![vec![1, 2, 3]]);
    }

    #[test]
    fn codec_encode_codes_reject_invalid_shape() {
        let err = audio_codes_from_flat_data(&[2, 2, 3], &[0; 12], 2, "codec_encode")
            .expect_err("batch must be one");

        assert!(err.to_string().contains("codec_encode"));
    }

    #[test]
    fn decode_step_state_initializes_from_streaming_metadata() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();

        assert_eq!(state.batch_size, 1);
        assert_eq!(state.transformer_offsets.len(), 1);
        assert_eq!(state.attention_caches.len(), 1);
        assert_eq!(state.transformer_offsets[0].shape, vec![1]);
        assert_eq!(state.transformer_offsets[0].data, vec![0]);
        assert_eq!(state.attention_caches[0].keys.shape, vec![1, 4, 8, 64]);
        assert_eq!(state.attention_caches[0].keys.data.len(), 4 * 8 * 64);
        assert!(state.input_names().contains(&"transformer_offset_0"));
        assert!(state.output_names().contains(&"attn_cached_values_out_0"));
    }

    #[test]
    fn decode_step_state_rejects_missing_streaming_metadata() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta_without_streaming(48_000, 2, 16);
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap_err();

        assert!(err.to_string().contains("streaming_decode"));
    }

    #[test]
    fn decode_step_state_rejects_bad_cache_dtype() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta_with_cache_dtype(48_000, 2, 16, "float16");
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap_err();

        assert!(err.to_string().contains("float32"));
        assert!(err.to_string().contains("float16"));
    }

    #[test]
    fn decode_step_state_updates_from_owned_outputs() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let mut state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();
        let mut outputs = HashMap::new();
        outputs.insert(
            "transformer_offset_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1],
                data: vec![3],
            },
        );
        outputs.insert(
            "attn_offset_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1],
                data: vec![4],
            },
        );
        outputs.insert(
            "attn_cached_keys_out_0".to_string(),
            OwnedTensorData::F32 {
                shape: vec![1, 4, 2, 64],
                data: vec![0.5; 4 * 2 * 64],
            },
        );
        outputs.insert(
            "attn_cached_values_out_0".to_string(),
            OwnedTensorData::F32 {
                shape: vec![1, 4, 2, 64],
                data: vec![0.25; 4 * 2 * 64],
            },
        );
        outputs.insert(
            "attn_cached_positions_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1, 2],
                data: vec![1, 2],
            },
        );

        state.update_from_owned_outputs(&outputs).unwrap();

        assert_eq!(state.transformer_offsets[0].data, vec![3]);
        assert_eq!(state.attention_caches[0].offset.data, vec![4]);
        assert_eq!(state.attention_caches[0].keys.shape, vec![1, 4, 2, 64]);
        assert_eq!(state.attention_caches[0].positions.data, vec![1, 2]);
    }

    #[test]
    fn decode_step_state_update_rejects_missing_output_name() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let mut state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();

        let err = state
            .update_from_owned_outputs(&HashMap::new())
            .unwrap_err();

        assert!(err.to_string().contains("codec_decode_step"));
        assert!(err.to_string().contains("transformer_offset_out_0"));
    }

    #[test]
    fn decode_step_buffered_reports_stage_when_unavailable() {
        let err = codec_decode_step_unavailable("fallback".to_string());

        assert!(err.to_string().contains("codec_decode_step"));
    }

    #[test]
    fn full_codec_decode_is_the_default_output_path() {
        const { assert!(!USE_CODEC_DECODE_STEP_BY_DEFAULT) };
    }

    #[test]
    fn pcm_chunk_buffer_concatenates_chunks_to_playback_result() {
        let mut buffer = PcmChunkBuffer::default();
        buffer.push_chunk(vec![0.1, 0.2]);
        buffer.push_chunk(Vec::new());
        buffer.push_chunk(vec![0.3, 0.4]);

        let result = buffer.into_tts_result().unwrap();

        assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);
        assert!(matches!(
            result.audio.pcm,
            PcmData::F32(samples) if samples == vec![0.1, 0.2, 0.3, 0.4]
        ));
    }

    #[test]
    fn pcm_chunk_buffer_rejects_unaligned_pcm() {
        let mut buffer = PcmChunkBuffer::default();
        buffer.push_chunk(vec![0.1, 0.2, 0.3]);

        let err = buffer.into_tts_result().unwrap_err();

        assert!(err.to_string().contains("stereo"));
    }

    #[test]
    fn frame_budget_starts_small_expands_and_flushes_to_metadata_limit() {
        let mut budget = FrameBudget::new(8, Some(10));

        assert_eq!(budget.next_batch_size(false), 1);
        budget.record_pcm_samples(960);
        assert_eq!(budget.next_batch_size(false), 2);
        budget.record_pcm_samples(960);
        assert_eq!(budget.next_batch_size(false), 4);
        budget.record_pcm_samples(1_920);
        assert_eq!(budget.next_batch_size(false), 8);
        assert_eq!(budget.next_batch_size(true), 8);
    }

    #[test]
    fn frame_budget_never_exceeds_metadata_batch_limit() {
        let mut budget = FrameBudget::new(2, Some(10));
        budget.record_pcm_samples(10_000);

        assert_eq!(budget.next_batch_size(false), 2);
        assert_eq!(budget.next_batch_size(true), 2);
    }

    #[test]
    fn streaming_audio_chunk_event_has_sequence_format_and_time_range() {
        let mut produced_samples = 0;
        let event =
            make_audio_chunk_event(3, vec![0.0, 0.1, 0.2, 0.3], &mut produced_samples, 5, true);

        let TtsSynthesisEvent::AudioChunk(chunk) = event else {
            panic!("expected audio chunk event");
        };
        assert_eq!(chunk.sequence, 3);
        assert_eq!(chunk.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(chunk.audio.channels, PLAYBACK_CHANNELS);
        assert_eq!(chunk.start_time_sec, Some(0.0));
        assert_eq!(
            chunk.end_time_sec,
            Some(2.0 / PLAYBACK_SAMPLE_RATE_HZ as f64)
        );
        assert_eq!(chunk.text_start, Some(0));
        assert_eq!(chunk.text_end, Some(5));
        assert!(chunk.is_final);
        assert!(matches!(chunk.audio.pcm, PcmData::F32(samples) if samples.len() == 4));
    }

    #[test]
    fn stream_session_events_emit_audio_chunks_before_end_with_monotonic_sequences() {
        let (events, result) = run_stubbed_stream_session(
            vec![
                stream_worker_chunk("hello", false),
                stream_worker_chunk("world", true),
            ],
            |chunk, emit| {
                emit(samples_for_text(&chunk.text, 0.1), false)?;
                emit(samples_for_text(&chunk.text, 0.2), true)?;
                Ok(tts_result(samples_for_text(&chunk.text, 0.3)))
            },
        )
        .unwrap();

        let end_index = events
            .iter()
            .position(|event| matches!(event, TtsSynthesisEvent::End(_)))
            .expect("stream should emit End");
        let audio_sequences = events
            .iter()
            .take(end_index)
            .filter_map(|event| match event {
                TtsSynthesisEvent::AudioChunk(chunk) => Some(chunk.sequence),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(audio_sequences, vec![1, 2, 3, 4]);
        assert!(matches!(events.last(), Some(TtsSynthesisEvent::End(_))));
        assert!(
            matches!(result.audio.pcm, PcmData::F32(samples) if samples == vec![
                0.3, 1.3, 0.3, 1.3, 0.3, 1.3, 0.3, 1.3,
                0.3, 1.3, 0.3, 1.3, 0.3, 1.3, 0.3, 1.3
            ])
        );
    }

    #[tokio::test]
    async fn stream_session_accepts_text_after_worker_starts() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let engine = MossOnnxTtsEngine::from_assets_for_test(assets);
        let mut session = MossStreamSession::new(&engine, TtsConfig::default());

        session
            .push_text(StreamingTextChunk::new("hello.").with_flush(true))
            .await
            .expect("first flushed chunk should start the worker");

        let result = session
            .push_text(StreamingTextChunk::final_chunk("world."))
            .await;

        assert!(
            !matches!(result, Err(TtsError::InvalidInput(ref message)) if message.contains("started")),
            "stream should accept chunks after processing has started: {result:?}"
        );
    }

    #[test]
    fn stream_session_finish_result_matches_end_event_result() {
        let (events, finish_result) =
            run_stubbed_stream_session(vec![stream_worker_chunk("hello", true)], |chunk, emit| {
                emit(samples_for_text(&chunk.text, 0.4), true)?;
                Ok(tts_result(samples_for_text(&chunk.text, 0.5)))
            })
            .unwrap();

        let end_result = events
            .iter()
            .find_map(|event| match event {
                TtsSynthesisEvent::End(result) => Some(result),
                _ => None,
            })
            .expect("stream should emit End result");

        assert_eq!(pcm_samples(&finish_result), pcm_samples(end_result));
    }

    #[test]
    fn stream_session_cancel_stops_new_audio_chunks_and_end() {
        let (chunks_tx, chunks_rx) = std::sync::mpsc::channel();
        chunks_tx
            .send(Some(stream_worker_chunk("hello", true)))
            .unwrap();
        chunks_tx.send(None).unwrap();
        drop(chunks_tx);
        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let flag_for_synth = Arc::clone(&cancel_flag);

        let err = synthesize_stream_chunks(
            chunks_rx,
            Arc::clone(&cancel_flag),
            events_tx,
            |_chunk, _emit| {
                flag_for_synth.store(true, Ordering::SeqCst);
                Err(cancelled_stream_error())
            },
        )
        .expect_err("cancelled stream should stop without End");

        assert!(err.to_string().contains("cancelled"));
        let events = drain_events(&mut events_rx);
        assert!(events.iter().all(|event| !matches!(
            event,
            TtsSynthesisEvent::AudioChunk(_) | TtsSynthesisEvent::End(_)
        )));
    }

    #[test]
    fn interleaves_codec_audio_from_channel_major_output() {
        let samples = interleave_codec_audio(
            &[1, 2, 3],
            &[0.1, 0.2, 0.3, 1.1, 1.2, 1.3],
            2,
            "codec_decode_step",
        )
        .unwrap();

        assert_eq!(samples, vec![0.1, 1.1, 0.2, 1.2]);
    }

    #[test]
    fn concatenates_multiple_chunk_results_in_order() {
        let result = concatenate_tts_results(vec![
            TtsResult {
                audio: AudioBuffer {
                    sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                    channels: PLAYBACK_CHANNELS,
                    pcm: PcmData::F32(vec![0.1, 0.2, 0.3, 0.4]),
                },
            },
            TtsResult {
                audio: AudioBuffer {
                    sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                    channels: PLAYBACK_CHANNELS,
                    pcm: PcmData::F32(vec![0.5, 0.6]),
                },
            },
        ])
        .unwrap();

        assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);
        assert!(matches!(
            result.audio.pcm,
            PcmData::F32(samples) if samples == vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]
        ));
    }

    #[test]
    fn rejects_empty_multi_chunk_concat() {
        let err = concatenate_tts_results(Vec::new()).expect_err("empty chunks should fail");

        assert!(err.to_string().contains("no audio"));
    }

    #[test]
    fn blocking_worker_path_propagates_session_init_errors() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let sessions = Arc::new(Mutex::new(None));
        let prepared = PreparedSynthesis {
            assets,
            prompt_audio_codes: vec![vec![1; 16]],
            chunks: vec![PreparedTextChunk {
                text: "hello".to_string(),
                token_ids: vec![1, 2, 3],
            }],
            sampling_mode: MossSamplingMode::Fixed,
            generation_config: MossGenerationConfig::default(),
            reference_audio: None,
        };

        let err = synthesize_prepared_with_sessions(&sessions, prepared)
            .expect_err("invalid fixture ONNX files must surface as worker init errors");

        assert!(err.to_string().contains("session_init"));
    }

    #[test]
    fn seeded_simple_rng_is_reproducible() {
        let mut first = SimpleRng::from_seed(42);
        let mut second = SimpleRng::from_optional_seed(Some(42));

        let first_values = (0..8).map(|_| first.next_f32()).collect::<Vec<_>>();
        let second_values = (0..8).map(|_| second.next_f32()).collect::<Vec<_>>();

        assert_eq!(first_values, second_values);
    }

    #[test]
    fn generation_config_overrides_frame_limit() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let explicit = MossGenerationConfig {
            seed: None,
            max_new_frames: Some(0),
        };

        assert_eq!(explicit.frame_limit(&assets), 0);
        assert_eq!(
            MossGenerationConfig::default().frame_limit(&assets),
            assets.max_new_frames()
        );
    }

    fn run_stubbed_stream_session<F>(
        chunks: Vec<StreamWorkerChunk>,
        mut synthesize_chunk: F,
    ) -> Result<(Vec<TtsSynthesisEvent>, TtsResult), MossTtsError>
    where
        F: FnMut(
            &PreparedTextChunk,
            &mut dyn FnMut(Vec<f32>, bool) -> Result<(), MossTtsError>,
        ) -> Result<TtsResult, MossTtsError>,
    {
        let (chunks_tx, chunks_rx) = std::sync::mpsc::channel();
        for chunk in chunks {
            chunks_tx.send(Some(chunk)).unwrap();
        }
        chunks_tx.send(None).unwrap();
        drop(chunks_tx);

        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let result = synthesize_stream_chunks(
            chunks_rx,
            Arc::new(AtomicBool::new(false)),
            events_tx,
            |chunk, emit| synthesize_chunk(chunk, emit),
        )?;

        Ok((drain_events(&mut events_rx), result))
    }

    fn drain_events(
        events_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TtsSynthesisEvent>,
    ) -> Vec<TtsSynthesisEvent> {
        let mut events = Vec::new();
        while let Ok(event) = events_rx.try_recv() {
            events.push(event);
        }
        events
    }

    fn stream_worker_chunk(text: &str, is_final: bool) -> StreamWorkerChunk {
        StreamWorkerChunk {
            chunk: PreparedTextChunk {
                text: text.to_string(),
                token_ids: vec![1, 2, 3],
            },
            is_final,
        }
    }

    fn samples_for_text(text: &str, base: f32) -> Vec<f32> {
        let frames = text.chars().count().max(1).min(4);
        let mut samples = Vec::with_capacity(frames * PLAYBACK_CHANNELS as usize);
        for _ in 0..frames {
            samples.push(base);
            samples.push(base + 1.0);
        }
        samples
    }

    fn tts_result(samples: Vec<f32>) -> TtsResult {
        TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(samples),
            },
        }
    }

    fn pcm_samples(result: &TtsResult) -> &[f32] {
        match &result.audio.pcm {
            PcmData::F32(samples) => samples,
            PcmData::I16(_) => panic!("stubbed stream should produce f32 PCM"),
        }
    }

    struct MossFixture {
        _temp: TempDir,
        tts_dir: PathBuf,
        codec_dir: PathBuf,
    }

    impl MossFixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let tts_dir = temp.path().join("tts");
            let codec_dir = temp.path().join("codec");
            std::fs::create_dir_all(&tts_dir).unwrap();
            std::fs::create_dir_all(&codec_dir).unwrap();
            for file in [
                "tokenizer.model",
                "prefill.onnx",
                "decode_step.onnx",
                "local_decoder.onnx",
                "local_cached_step.onnx",
                "local_fixed_sampled_frame.onnx",
                "tts_shared.data",
            ] {
                std::fs::write(tts_dir.join(file), b"x").unwrap();
            }
            for file in [
                "encode.onnx",
                "decode_full.onnx",
                "decode_step.onnx",
                "codec_shared.data",
            ] {
                std::fs::write(codec_dir.join(file), b"x").unwrap();
            }
            let fixture = Self {
                _temp: temp,
                tts_dir,
                codec_dir,
            };
            fixture.write_manifest(
                "tts_browser_onnx_meta.json",
                "../codec/codec_browser_onnx_meta.json",
                "tokenizer.model",
            );
            fixture.write_tts_meta();
            fixture.write_codec_meta(48_000, 2, 16);
            fixture
        }

        fn write_manifest(&self, tts_meta: &str, codec_meta: &str, tokenizer: &str) {
            std::fs::write(
                self.tts_dir.join(MANIFEST_FILE),
                format!(
                    r#"{{
  "format_version": 1,
  "model_files": {{
    "tts_meta": "{tts_meta}",
    "codec_meta": "{codec_meta}",
    "tokenizer_model": "{tokenizer}"
  }},
  "tts_config": {{
    "n_vq": 16,
    "audio_pad_token_id": 1024,
    "audio_start_token_id": 6,
    "audio_end_token_id": 7,
    "audio_user_slot_token_id": 8,
    "audio_assistant_slot_token_id": 9,
    "audio_codebook_sizes": [1024]
  }},
  "prompt_templates": {{
    "user_prompt_prefix_token_ids": [1, 2],
    "user_prompt_after_reference_token_ids": [3, 4],
    "assistant_prompt_prefix_token_ids": [5]
  }},
  "generation_defaults": {{ "max_new_frames": 4 }},
  "builtin_voices": [
    {{ "voice": "Junhao", "prompt_audio_codes": [[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]] }},
    {{ "voice": "Ava", "prompt_audio_codes": [[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]] }}
  ]
}}"#
                ),
            )
            .unwrap();
        }

        fn write_tts_meta(&self) {
            std::fs::write(
                self.tts_dir.join("tts_browser_onnx_meta.json"),
                r#"{
  "format_version": 1,
  "files": {
    "prefill": "prefill.onnx",
    "decode_step": "decode_step.onnx",
    "local_decoder": "local_decoder.onnx",
    "local_cached_step": "local_cached_step.onnx",
    "local_fixed_sampled_frame": "local_fixed_sampled_frame.onnx"
  },
  "external_data_files": {
    "prefill.onnx": ["tts_shared.data"],
    "decode_step.onnx": ["tts_shared.data"]
  },
  "onnx": {
    "prefill_output_names": ["global_hidden"],
    "decode_input_names": ["input_ids"],
    "decode_output_names": ["global_hidden"],
    "local_cached_input_names": ["global_hidden", "text_token_id"],
    "local_cached_output_names": ["text_logits", "audio_logits"]
  }
}"#,
            )
            .unwrap();
        }

        fn write_codec_meta(&self, sample_rate: u32, channels: u16, num_quantizers: u32) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, Some("float32"));
        }

        fn write_codec_meta_without_streaming(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
        ) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, None);
        }

        fn write_codec_meta_with_cache_dtype(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
            cache_dtype: &str,
        ) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, Some(cache_dtype));
        }

        fn write_codec_meta_inner(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
            streaming_cache_dtype: Option<&str>,
        ) {
            let streaming_decode = streaming_cache_dtype
                .map(|cache_dtype| {
                    format!(
                        r#",
  "streaming_decode": {{
    "batch_size": 1,
    "transformer_offsets": [
      {{
        "input_name": "transformer_offset_0",
        "output_name": "transformer_offset_out_0",
        "shape": [1],
        "dtype": "int32"
      }}
    ],
    "attention_caches": [
      {{
        "offset_input_name": "attn_offset_0",
        "offset_output_name": "attn_offset_out_0",
        "cached_keys_input_name": "attn_cached_keys_0",
        "cached_keys_output_name": "attn_cached_keys_out_0",
        "cached_values_input_name": "attn_cached_values_0",
        "cached_values_output_name": "attn_cached_values_out_0",
        "cached_positions_input_name": "attn_cached_positions_0",
        "cached_positions_output_name": "attn_cached_positions_out_0",
        "offset_shape": [1],
        "cache_shape": [1, 4, 8, 64],
        "positions_shape": [1, 8],
        "cache_dtype": "{cache_dtype}",
        "positions_dtype": "int32"
      }}
    ]
  }}"#
                    )
                })
                .unwrap_or_default();
            std::fs::write(
                self.codec_dir.join("codec_browser_onnx_meta.json"),
                format!(
                    r#"{{
  "format_version": 2,
  "files": {{ "encode": "encode.onnx", "decode_full": "decode_full.onnx", "decode_step": "decode_step.onnx" }},
  "external_data_files": {{
    "encode.onnx": ["codec_shared.data"],
    "decode_full.onnx": ["codec_shared.data"],
    "decode_step.onnx": ["codec_shared.data"]
  }},
  "codec_config": {{ "sample_rate": {sample_rate}, "channels": {channels}, "num_quantizers": {num_quantizers} }},
  "onnx": {{
    "encode_input_names": ["waveform", "input_lengths"],
    "encode_output_names": ["audio_codes", "audio_code_lengths"],
    "decode_input_names": ["audio_codes", "audio_code_lengths"],
    "decode_output_names": ["audio", "audio_lengths"],
    "decode_step_input_names": ["audio_codes", "audio_code_lengths"],
    "decode_step_output_names": ["audio", "audio_lengths"]
  }}{streaming_decode}
}}"#
                ),
            )
            .unwrap();
        }
    }
}
