use std::{path::Path, time::Instant};

use tts_core::{
    MossTtsConfig, PcmData, TtsConfig, TtsEngine, TtsStreamConfig, TtsSynthesisEvent,
    PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ,
};
use tts_moss::MossOnnxTtsEngine;

fn write_wav(path: &Path, result: &tts_core::TtsResult) -> hound::Result<()> {
    let spec = hound::WavSpec {
        channels: result.audio.channels,
        sample_rate: result.audio.sample_rate_hz,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    match &result.audio.pcm {
        PcmData::I16(samples) => {
            for sample in samples {
                writer.write_sample(*sample)?;
            }
        }
        PcmData::F32(samples) => {
            for sample in samples {
                let clamped = sample.clamp(-1.0, 1.0);
                writer.write_sample((clamped * i16::MAX as f32) as i16)?;
            }
        }
    }
    writer.finalize()
}

#[tokio::test]
#[ignore = "requires downloaded MOSS ONNX model assets and measures local realtime streaming performance"]
async fn streaming_synthesis_is_realtime_on_this_machine() {
    let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
        .expect("set MOSS_TTS_MODEL_DIR to MOSS-TTS-Nano-100M-ONNX before running this test");
    eprintln!("MOSS_TTS_MODEL_DIR={model_dir}");

    let text = std::env::var("MOSS_TTS_PERF_TEXT")
        .unwrap_or_else(|_| "你好，当前测试用于确认本机是否支持实时语音合成播放。".to_string());
    let max_rtf = std::env::var("MOSS_TTS_REALTIME_MAX_RTF")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(1.0);
    let chunk_ms = std::env::var("MOSS_TTS_PERF_CHUNK_MS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(240);
    let warmup_text =
        std::env::var("MOSS_TTS_PERF_WARMUP_TEXT").unwrap_or_else(|_| "预热。".to_string());

    let engine = MossOnnxTtsEngine::from_env().expect("MOSS engine should initialize");
    assert!(engine
        .health_check()
        .await
        .expect("health check should pass"));

    let warmup_started = Instant::now();
    let warmup_result = engine
        .synthesize_stream_events(
            &warmup_text,
            perf_config(chunk_ms),
            Box::new(|_| {}),
        )
        .await
        .expect("MOSS warmup streaming synthesis should succeed");
    warmup_result
        .validate_for_playback()
        .expect("warmup audio must satisfy playback contract");
    eprintln!(
        "streaming warmup: text_chars={} elapsed={:.3}s audio={:.3}s",
        warmup_text.chars().count(),
        warmup_started.elapsed().as_secs_f64(),
        audio_duration_sec(&warmup_result)
    );

    let mut first_audio_elapsed = None;
    let mut chunk_count = 0usize;
    let started = Instant::now();
    let result = engine
        .synthesize_stream_events(
            &text,
            perf_config(chunk_ms),
            Box::new(|event| {
                if let TtsSynthesisEvent::AudioChunk(_) = event {
                    chunk_count += 1;
                    first_audio_elapsed.get_or_insert_with(|| started.elapsed());
                }
            }),
        )
        .await
        .expect("MOSS streaming synthesis should succeed");
    let elapsed = started.elapsed();

    result
        .validate_for_playback()
        .expect("synthesized audio must satisfy playback contract");
    assert!(
        result.audio.pcm.len_frames(result.audio.channels) > 0,
        "synthesized audio should contain PCM frames"
    );
    assert!(chunk_count > 0, "streaming synthesis should emit audio chunks");

    let audio_duration_sec = audio_duration_sec(&result);
    let elapsed_sec = elapsed.as_secs_f64();
    let rtf = elapsed_sec / audio_duration_sec;
    eprintln!(
        "streaming realtime perf: text_chars={} chunk_ms={} chunks={} first_audio={:?} elapsed={:.3}s audio={:.3}s rtf={:.3} max_rtf={:.3}",
        text.chars().count(),
        chunk_ms,
        chunk_count,
        first_audio_elapsed,
        elapsed_sec,
        audio_duration_sec,
        rtf,
        max_rtf
    );

    assert!(
        rtf <= max_rtf,
        "streaming synthesis is not realtime: rtf={rtf:.3}, max_rtf={max_rtf:.3}"
    );
}

fn perf_config(chunk_ms: u32) -> TtsConfig {
    TtsConfig {
        stream: Some(TtsStreamConfig {
            audio_chunk_ms: Some(chunk_ms),
            ..TtsStreamConfig::default()
        }),
        moss: Some(MossTtsConfig {
            sampling_mode: Some("greedy".to_string()),
            ..MossTtsConfig::default()
        }),
        ..TtsConfig::default()
    }
}

fn audio_duration_sec(result: &tts_core::TtsResult) -> f64 {
    result.audio.pcm.len_frames(result.audio.channels) as f64 / result.audio.sample_rate_hz as f64
}

#[tokio::test]
#[ignore = "requires downloaded MOSS ONNX model assets and validates streaming codec decode"]
async fn synthesizes_playback_ready_audio_with_greedy_streaming_decode() {
    let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
        .expect("set MOSS_TTS_MODEL_DIR to MOSS-TTS-Nano-100M-ONNX before running this test");
    eprintln!("MOSS_TTS_MODEL_DIR={model_dir}");

    let engine = MossOnnxTtsEngine::from_env().expect("MOSS engine should initialize");
    assert!(engine
        .health_check()
        .await
        .expect("health check should pass"));

    let result = engine
        .synthesize(
            "Streaming decode test.",
            TtsConfig {
                moss: Some(MossTtsConfig {
                    sampling_mode: Some("greedy".to_string()),
                    ..MossTtsConfig::default()
                }),
                ..TtsConfig::default()
            },
        )
        .await
        .expect("MOSS greedy synthesis with streaming decode should succeed or fallback cleanly");

    result
        .validate_for_playback()
        .expect("synthesized audio must satisfy playback contract");
    assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
    assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);
    assert!(result.audio.pcm.len_frames(result.audio.channels) > 0);
}

#[tokio::test]
#[ignore = "requires downloaded MOSS ONNX model assets and runs full inference"]
async fn synthesizes_playback_ready_audio() {
    let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
        .expect("set MOSS_TTS_MODEL_DIR to MOSS-TTS-Nano-100M-ONNX before running this test");
    eprintln!("MOSS_TTS_MODEL_DIR={model_dir}");

    let started = Instant::now();
    let engine = MossOnnxTtsEngine::from_env().expect("MOSS engine should initialize");
    assert_eq!(engine.engine_name(), "moss-onnx-tts");
    assert!(engine
        .health_check()
        .await
        .expect("health check should pass"));
    eprintln!("model load/health check elapsed: {:?}", started.elapsed());

    let synth_started = Instant::now();
    let text =
        std::env::var("MOSS_TTS_TEXT").unwrap_or_else(|_| "你好，欢迎使用语音编程。".to_string());
    let result = engine
        .synthesize(&text, TtsConfig::default())
        .await
        .expect("MOSS synthesis should succeed");
    let synth_elapsed = synth_started.elapsed();

    result
        .validate_for_playback()
        .expect("synthesized audio must satisfy playback contract");
    assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
    assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);

    let frames = result.audio.pcm.len_frames(result.audio.channels);
    assert!(frames > 0, "synthesized audio should contain PCM frames");
    let duration_secs = frames as f64 / result.audio.sample_rate_hz as f64;
    let sample_format = match &result.audio.pcm {
        PcmData::I16(samples) => {
            assert!(!samples.is_empty(), "i16 PCM should not be empty");
            "i16"
        }
        PcmData::F32(samples) => {
            assert!(!samples.is_empty(), "f32 PCM should not be empty");
            assert!(
                samples.iter().all(|sample| sample.is_finite()),
                "PCM samples must be finite"
            );
            "f32"
        }
    };

    eprintln!(
        "synthesis elapsed: {:?}; format: {sample_format}; frames: {frames}; duration: {:.3}s",
        synth_elapsed, duration_secs
    );

    if let Ok(output_path) = std::env::var("MOSS_TTS_OUTPUT_WAV") {
        write_wav(Path::new(&output_path), &result).expect("should write synthesized audio as WAV");
        eprintln!("wrote synthesized WAV: {output_path}");
    }
}
