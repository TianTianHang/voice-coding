use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::AudioRecorder;
use crate::vad::{VadState, SAMPLE_RATE};
use stt_core::SttEngine;

pub struct VadRecorderState {
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
}

impl VadRecorderState {
    pub fn new() -> Self {
        Self {
            recorder: Arc::new(Mutex::new(None)),
        }
    }
}

fn get_vad_lib_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    // Detect platform and library filename
    let (platform, lib_name) = if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            ("Linux/x64", "libten_vad.so")
        } else if cfg!(target_arch = "aarch64") {
            ("Linux/arm64", "libten_vad.so")
        } else {
            return Err(format!("Unsupported Linux architecture"));
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            ("macOS/x86_64", "libten_vad.dylib")
        } else if cfg!(target_arch = "aarch64") {
            ("macOS/arm64", "libten_vad.dylib")
        } else {
            return Err(format!("Unsupported macOS architecture"));
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            ("Windows/x64", "ten_vad.dll")
        } else if cfg!(target_arch = "x86") {
            ("Windows/x86", "ten_vad.dll")
        } else {
            return Err(format!("Unsupported Windows architecture"));
        }
    } else {
        return Err(format!("Unsupported operating system"));
    };

    let lib_path = format!("libs/{}/{}", platform, lib_name);

    // Try resource directory first (production build)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let path = resource_dir.join(&lib_path);
        if path.exists() {
            return Ok(path);
        }
    }

    // Fallback to development path
    let dev_path = std::path::PathBuf::from(&lib_path);
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err(format!(
        "{} not found (tried resource dir and {:?})",
        lib_name, dev_path
    ))
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample as u32 / 8);
    let block_align: u16 = num_channels * (bits_per_sample / 8);
    let data_size = samples.len() * (bits_per_sample as usize / 8);
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(file_size as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    for &sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    wav
}

async fn transcribe_audio_internal(_app: AppHandle, wav_data: Vec<u8>) -> Result<String, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        let input = stt_core::AudioInput::Bytes(wav_data);
        let config = stt_core::SttConfig {
            language: None,
            ..Default::default()
        };

        let engine = crate::asr::get_stt_engine();
        let result = engine
            .transcribe(input, config)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result.text)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = (app, wav_data);
        Err("STT engine not available".into())
    }
}

#[tauri::command]
pub async fn start_listening(
    app: AppHandle,
    state: tauri::State<'_, VadRecorderState>,
) -> Result<(), String> {
    {
        let recorder = state.recorder.lock();
        if recorder.is_some() {
            return Err("Already listening".into());
        }
    }

    let lib_path = get_vad_lib_path(&app)?;
    let recorder = AudioRecorder::new(&lib_path).map_err(|e| e.to_string())?;

    let sm = recorder.state_machine();
    let event_rx = recorder.event_rx();

    {
        let mut guard = state.recorder.lock();
        *guard = Some(recorder);
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = event_rx.recv() {
            match event {
                crate::vad::VadEvent::StateChanged(s) => {
                    let _ =
                        app_clone.emit("vad-state", serde_json::json!({ "state": s.to_string() }));
                }
                crate::vad::VadEvent::SpeechDetected(audio_data) => {
                    let wav = encode_wav(&audio_data, SAMPLE_RATE);
                    match transcribe_audio_internal(app_clone.clone(), wav).await {
                        Ok(text) => {
                            let _ =
                                app_clone.emit("transcript", serde_json::json!({ "text": text }));
                        }
                        Err(e) => {
                            let _ = app_clone.emit("error", &e);
                        }
                    }
                    let mut sm_guard = sm.lock();
                    sm_guard.finish_transcription();
                }
                crate::vad::VadEvent::Error(msg) => {
                    let _ = app_clone.emit("error", &msg);
                    let mut sm_guard = sm.lock();
                    sm_guard.stop();
                }
            }
        }
    });

    {
        let guard = state.recorder.lock();
        if let Some(rec) = guard.as_ref() {
            let sm = rec.state_machine();
            let mut sm = sm.lock();
            sm.start();
        }
    }

    Ok(())
}

#[tauri::command]
pub fn stop_listening(state: tauri::State<'_, VadRecorderState>) -> Result<(), String> {
    let mut guard = state.recorder.lock();
    if let Some(recorder) = guard.take() {
        let sm = recorder.state_machine();
        let mut sm = sm.lock();
        sm.stop();
    }
    Ok(())
}

#[tauri::command]
pub fn get_vad_state(state: tauri::State<'_, VadRecorderState>) -> Result<String, String> {
    let guard = state.recorder.lock();
    match guard.as_ref() {
        Some(recorder) => {
            let sm = recorder.state_machine();
            let sm = sm.lock();
            Ok(sm.get_state().to_string())
        }
        None => Ok(VadState::Idle.to_string()),
    }
}
