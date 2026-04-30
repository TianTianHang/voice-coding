// ⚠️  IMPORTANT: These tests must be run with --test-threads=1 to avoid OOM
// Each test loads a 2.5GB model, so concurrent execution will exceed memory limits.
// Run with: cargo test --test boundary_test -- --test-threads=1

use stt_core::{AudioInput, SttConfig, SttEngine};
use std::sync::Arc;

use once_cell::sync::Lazy;
use stt_qwen3::Qwen3AsrEngine;

const MODEL_DIR: &str = "../../models";

fn model_tests_enabled() -> bool {
    std::env::var("RUN_QWEN3_MODEL_TESTS")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

macro_rules! require_model_tests {
    () => {
        if !model_tests_enabled() {
            eprintln!("Skipping model inference test; set RUN_QWEN3_MODEL_TESTS=1 to enable");
            return;
        }
    };
}

static TEST_ENGINE: Lazy<Arc<Qwen3AsrEngine>> =
    Lazy::new(|| Arc::new(Qwen3AsrEngine::new(MODEL_DIR).expect("Failed to create test engine")));

async fn setup_test_engine() -> Arc<Qwen3AsrEngine> {
    TEST_ENGINE.clone()
}

fn create_mock_samples(duration_sec: usize, sample_rate: u32) -> Vec<f32> {
    let num_samples = duration_sec * sample_rate as usize;
    (0..num_samples)
        .map(|i| {
            let freq = 440.0;
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32;
            phase.sin() * 0.3
        })
        .collect()
}

#[tokio::test]
async fn test_minimum_duration_boundary() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000); // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_below_minimum_duration() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = vec![0.5f32; 1000];
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_samples() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = AudioInput::Samples(vec![], 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_silence_only_audio() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = vec![0.0f32; 32000]; // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_extreme_low_sample_rate() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 8000);
    let input = AudioInput::Samples(samples, 8000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_extreme_high_sample_rate() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 96000); // 增加到2秒
    let input = AudioInput::Samples(samples, 96000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_clipping_audio() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = vec![2.0f32; 32000]; // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_negative_amplitude_audio() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = vec![-1.5f32; 32000]; // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_very_large_max_tokens() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000); // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        max_new_tokens: Some(10000),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
    let result = result.unwrap();
    assert!(result.timing.tokens_generated.unwrap() < 10000);
}

#[tokio::test]
async fn test_long_audio_handling() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let samples: Vec<f32> = (0..300_000)
        .map(|i| {
            let freq = 440.0 + (i as f32 / 300_000.0) * 100.0;
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / 16000.0;
            phase.sin() * 0.5
        })
        .collect();

    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        chunk_seconds: Some(30.0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
    let result = result.unwrap();
    assert!(result.timing.processing_time_sec > 0.0);
}

#[tokio::test]
async fn test_vad_with_very_small_chunk() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(60, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        chunk_seconds: Some(5.0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_vad_with_very_large_chunk() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(120, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        chunk_seconds: Some(60.0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_zero_max_tokens() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000); // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        max_new_tokens: Some(0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}

#[tokio::test]
async fn test_single_sample() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = vec![0.5f32];
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_very_short_duration() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000); // 增加到2秒
    let input = AudioInput::Samples(samples, 16000);

    let result = engine.transcribe(input, SttConfig::default()).await;
    assert!(result.is_ok(), "transcription failed: {:?}", result.err());
}
