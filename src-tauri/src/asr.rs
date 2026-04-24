use stt_core::{AudioInput, SttConfig, SttEngine};
#[cfg(feature = "stt-qwen3")]
use stt_qwen3::Qwen3AsrEngine;
#[cfg(feature = "stt-qwen3")]
use once_cell::sync::Lazy;
use std::io::Write;

#[cfg(feature = "stt-qwen3")]
static STT_ENGINE: Lazy<Qwen3AsrEngine> = Lazy::new(|| {
    let model_dir = std::env::var("STT_MODEL_DIR")
        .unwrap_or_else(|_| "models".to_string());
    Qwen3AsrEngine::new(&model_dir)
        .expect("Failed to initialize STT engine")
});

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("voice-coding")
}

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

#[tauri::command]
pub async fn transcribe_audio_data(
    audio_data: Vec<u8>,
    language: Option<String>,
) -> Result<String, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let file_name = format!("{}.wav", uuid::Uuid::new_v4());
        let file_path = dir.join(file_name);

        let mut file = std::fs::File::create(&file_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(&audio_data)
            .map_err(|e| format!("Failed to write audio data: {}", e))?;
        drop(file);

        let input = AudioInput::FilePath(file_path.to_string_lossy().to_string());
        let config = SttConfig {
            language,
            ..Default::default()
        };

        let result = (*STT_ENGINE).transcribe(input, config)
            .await
            .map_err(|e| e.to_string())?;

        let _ = std::fs::remove_file(&file_path);

        Ok(result.text)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = (audio_data, language);
        Err("STT engine not available: no engine feature enabled".into())
    }
}
