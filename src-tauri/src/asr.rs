use stt_core::{AudioInput, SttConfig, SttEngine};
#[cfg(feature = "stt-qwen3")]
use stt_qwen3::Qwen3AsrEngine;
#[cfg(feature = "stt-qwen3")]
use once_cell::sync::Lazy;

#[cfg(feature = "stt-qwen3")]
static STT_ENGINE: Lazy<Qwen3AsrEngine> = Lazy::new(|| {
    let model_dir = std::env::var("STT_MODEL_DIR")
        .unwrap_or_else(|_| "models".to_string());
    Qwen3AsrEngine::new(&model_dir)
        .expect("Failed to initialize STT engine")
});

#[tauri::command]
pub async fn transcribe(
    audio_path: String,
    language: Option<String>,
) -> Result<String, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        let input = AudioInput::FilePath(audio_path);
        let config = SttConfig {
            language,
            ..Default::default()
        };

        let result = (*STT_ENGINE).transcribe(input, config)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result.text)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = (audio_path, language);
        Err("STT engine not available: no engine feature enabled".into())
    }
}
