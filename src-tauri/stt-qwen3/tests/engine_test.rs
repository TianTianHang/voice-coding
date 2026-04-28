use std::path::Path;
use std::sync::Arc;

use once_cell::sync::Lazy;
use stt_core::{AudioInput, SttConfig, SttEngine, SttError};
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

static TEST_ENGINE: Lazy<Result<Arc<Qwen3AsrEngine>, SttError>> =
    Lazy::new(|| Qwen3AsrEngine::new(MODEL_DIR).map(Arc::new));

async fn setup_test_engine() -> Arc<Qwen3AsrEngine> {
    TEST_ENGINE
        .as_ref()
        .expect("Failed to create shared test engine")
        .clone()
}

fn create_mock_samples(duration_sec: usize, sample_rate: u32) -> Vec<f32> {
    let num_samples = duration_sec * sample_rate as usize;
    (0..num_samples)
        .map(|i| {
            let freq = 440.0; // A4 note
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32;
            phase.sin() * 0.3
        })
        .collect()
}

fn create_mock_input() -> AudioInput {
    let samples = create_mock_samples(2, 16000); // 2 seconds at 16kHz
    AudioInput::Samples(samples, 16000)
}

#[tokio::test]
async fn test_engine_initialization_success() {
    require_model_tests!();
    let engine = Qwen3AsrEngine::new(MODEL_DIR).unwrap();
    assert_eq!(engine.engine_name(), "qwen3-asr-0.6b");
    assert_eq!(engine.supported_languages().len(), 30);
    assert!(engine.supported_languages().contains(&"zh"));
    assert!(engine.supported_languages().contains(&"en"));
}

#[tokio::test]
async fn test_engine_initialization_invalid_path() {
    let result = Qwen3AsrEngine::new("/nonexistent/path");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_supported_language_accepted() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let config = SttConfig {
        language: Some("zh".to_string()),
        ..Default::default()
    };

    let input = create_mock_input();
    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unsupported_language_rejected() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let config = SttConfig {
        language: Some("xx".to_string()),
        ..Default::default()
    };

    let input = create_mock_input();
    let result = engine.transcribe(input, config).await;
    assert!(matches!(result, Err(SttError::UnsupportedLanguage(_))));
}

#[tokio::test]
async fn test_unsupported_language_another_case() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let config = SttConfig {
        language: Some("invalid".to_string()),
        ..Default::default()
    };

    let input = create_mock_input();
    let result = engine.transcribe(input, config).await;
    assert!(matches!(result, Err(SttError::UnsupportedLanguage(_))));
}

#[tokio::test]
async fn test_audio_input_filepath_valid() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let test_audio_path = Path::new("../../test_audio/librispeech_0_1089_0.wav");
    if !test_audio_path.exists() {
        return;
    }

    let input = AudioInput::FilePath(test_audio_path.to_string_lossy().to_string());
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_audio_input_filepath_missing() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = AudioInput::FilePath("/nonexistent/file.wav".to_string());
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(matches!(result, Err(SttError::AudioLoadError(_))));
}

#[tokio::test]
async fn test_audio_input_bytes_valid() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let test_audio_path = Path::new("../../test_audio/librispeech_1_1089_1.wav");
    if !test_audio_path.exists() {
        return;
    }

    let audio_data = std::fs::read(test_audio_path).unwrap();
    let input = AudioInput::Bytes(audio_data);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_audio_input_bytes_invalid() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = AudioInput::Bytes(vec![0u8; 100]);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_audio_input_samples_16khz() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_audio_input_samples_resampling() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 48000);
    let input = AudioInput::Samples(samples, 48000);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_audio_input_samples_8khz() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 8000);
    let input = AudioInput::Samples(samples, 8000);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_vad_not_triggered_under_threshold() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let samples = create_mock_samples(44, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_vad_triggered_at_threshold() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let samples = create_mock_samples(45, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        chunk_seconds: Some(30.0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(!result.text.is_empty());
}

#[tokio::test]
async fn test_vad_disabled() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let samples = create_mock_samples(50, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: false,
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_config_max_new_tokens() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = create_mock_input();
    let config = SttConfig {
        max_new_tokens: Some(50),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.timing.tokens_generated.unwrap() <= 50);
}

#[tokio::test]
async fn test_config_chunk_seconds() {
    require_model_tests!();
    let engine = setup_test_engine().await;

    let samples = create_mock_samples(60, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig {
        enable_vad: true,
        chunk_seconds: Some(15.0),
        ..Default::default()
    };

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_health_check_success() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let result = engine.health_check().await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_health_check_missing_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp_dir.path().join("onnx_models")).unwrap();

    match Qwen3AsrEngine::new(temp_dir.path().to_str().unwrap()) {
        Ok(engine) => {
            let health_result = engine.health_check().await;
            assert!(health_result.is_err());
        }
        Err(_) => {}
    }
}

#[tokio::test]
async fn test_transcription_result_structure() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = create_mock_input();
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
    let result = result.unwrap();

    assert!(!result.text.is_empty());
    assert!(result.timing.audio_duration_sec > 0.0);
    assert!(result.timing.processing_time_sec > 0.0);
    assert!(result.timing.rtf > 0.0);
    assert!(result.timing.tokens_generated.is_some());
}

#[tokio::test]
async fn test_multiple_languages() {
    require_model_tests!();
    let engine = setup_test_engine().await;
    let input = create_mock_input();

    for lang in &["zh", "en", "ja", "ko", "fr"] {
        let config = SttConfig {
            language: Some(lang.to_string()),
            ..Default::default()
        };

        let result = engine.transcribe(input.clone(), config).await;
        assert!(result.is_ok(), "Language {} should be supported", lang);
    }
}
