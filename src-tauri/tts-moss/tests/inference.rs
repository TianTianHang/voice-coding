use std::{path::Path, time::Instant};

use tts_core::{
    MossTtsConfig, PcmData, TtsConfig, TtsEngine, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ,
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
#[ignore = "requires downloaded MOSS ONNX model assets and validates streaming codec decode"]
async fn synthesizes_playback_ready_audio_with_greedy_streaming_decode() {
    let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
        .expect("set MOSS_TTS_MODEL_DIR to MOSS-TTS-Nano-100M-ONNX before running this test");
    eprintln!("MOSS_TTS_MODEL_DIR={model_dir}");

    let engine = MossOnnxTtsEngine::from_env().expect("MOSS engine should initialize");
    assert!(engine.health_check().await.expect("health check should pass"));

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
    assert!(engine.health_check().await.expect("health check should pass"));
    eprintln!("model load/health check elapsed: {:?}", started.elapsed());

    let synth_started = Instant::now();
    let result = engine
        .synthesize("你好，欢迎使用语音编程。", TtsConfig::default())
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
            assert!(samples.iter().all(|sample| sample.is_finite()), "PCM samples must be finite");
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
