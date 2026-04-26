#[cfg(feature = "stt-qwen3")]
use once_cell::sync::Lazy;
use std::io::Write;
use std::path::Path;
use stt_core::{AudioInput, SttConfig, SttEngine};
#[cfg(feature = "stt-qwen3")]
use stt_qwen3::Qwen3AsrEngine;

#[cfg(feature = "stt-qwen3")]
static STT_ENGINE: Lazy<Qwen3AsrEngine> = Lazy::new(|| {
    let model_dir = std::env::var("STT_MODEL_DIR").unwrap_or_else(|_| "models".to_string());
    Qwen3AsrEngine::new(&model_dir).expect("Failed to initialize STT engine")
});

#[cfg(feature = "stt-qwen3")]
pub fn get_stt_engine() -> &'static Qwen3AsrEngine {
    &STT_ENGINE
}

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("voice-coding")
}

fn cleanup_temp_audio_file(file_path: &Path) -> Result<(), String> {
    match std::fs::remove_file(file_path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "Failed to remove temp file {}: {}",
            file_path.display(),
            e
        )),
    }
}

#[tauri::command]
pub async fn transcribe(audio_path: String, language: Option<String>) -> Result<String, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        let input = AudioInput::FilePath(audio_path);
        let config = SttConfig {
            language,
            ..Default::default()
        };

        let result = (*STT_ENGINE)
            .transcribe(input, config)
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

        let transcription_result = (*STT_ENGINE)
            .transcribe(input, config)
            .await
            .map(|result| result.text)
            .map_err(|e| e.to_string());

        let cleanup_result = cleanup_temp_audio_file(&file_path);

        match (transcription_result, cleanup_result) {
            (Ok(text), Ok(())) => Ok(text),
            (Ok(_), Err(cleanup_err)) => Err(cleanup_err),
            (Err(transcription_err), _) => Err(transcription_err),
        }
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = (audio_data, language);
        Err("STT engine not available: no engine feature enabled".into())
    }
}

#[cfg(test)]
mod tests {
    use super::cleanup_temp_audio_file;

    #[test]
    fn cleanup_temp_audio_file_removes_existing_file() {
        let temp_file = std::env::temp_dir().join(format!("voice-coding-test-{}.wav", uuid::Uuid::new_v4()));
        std::fs::write(&temp_file, b"wav").expect("failed to create temp file");

        let result = cleanup_temp_audio_file(&temp_file);
        assert!(result.is_ok());
        assert!(!temp_file.exists());
    }

    #[test]
    fn cleanup_temp_audio_file_ignores_missing_file() {
        let temp_file = std::env::temp_dir().join(format!("voice-coding-test-missing-{}.wav", uuid::Uuid::new_v4()));

        let result = cleanup_temp_audio_file(&temp_file);
        assert!(result.is_ok());
    }
}
