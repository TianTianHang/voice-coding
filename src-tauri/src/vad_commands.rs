use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::AudioRecorder;
use crate::vad::{VadConfig, VadState, SAMPLE_RATE, THRESHOLD};
use stt_core::{AudioInput, SttEngine};

pub struct VadRecorderState {
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    active_session: Arc<Mutex<Option<u64>>>,
    starting: Arc<Mutex<bool>>,
    next_session_id: AtomicU64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VadRuntimeConfig {
    pub threshold: f32,
}

impl Default for VadRuntimeConfig {
    fn default() -> Self {
        Self {
            threshold: THRESHOLD,
        }
    }
}

pub struct VadRuntimeConfigState {
    config: Arc<Mutex<VadRuntimeConfig>>,
}

impl VadRuntimeConfigState {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(VadRuntimeConfig::default())),
        }
    }

    fn get(&self) -> VadRuntimeConfig {
        self.config.lock().clone()
    }

    fn set(&self, config: VadRuntimeConfig) {
        *self.config.lock() = config;
    }
}

impl VadRecorderState {
    pub fn new() -> Self {
        Self {
            recorder: Arc::new(Mutex::new(None)),
            active_session: Arc::new(Mutex::new(None)),
            starting: Arc::new(Mutex::new(false)),
            next_session_id: AtomicU64::new(1),
        }
    }

    fn allocate_session_id(&self) -> u64 {
        self.next_session_id.fetch_add(1, Ordering::Relaxed)
    }

    fn set_active_session(&self, session_id: u64) {
        let mut guard = self.active_session.lock();
        *guard = Some(session_id);
    }

    fn clear_active_session(&self) -> Option<u64> {
        let mut guard = self.active_session.lock();
        guard.take()
    }

    fn current_active_session(&self) -> Option<u64> {
        *self.active_session.lock()
    }
}

fn is_session_active(active_session: &Arc<Mutex<Option<u64>>>, session_id: u64) -> bool {
    *active_session.lock() == Some(session_id)
}

fn get_vad_lib_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    // Detect platform and library filename
    let (platform, lib_name) = if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            ("Linux/x64", "libten_vad.so")
        } else if cfg!(target_arch = "aarch64") {
            ("Linux/arm64", "libten_vad.so")
        } else {
            return Err("Unsupported Linux architecture".to_string());
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            ("macOS/x86_64", "libten_vad.dylib")
        } else if cfg!(target_arch = "aarch64") {
            ("macOS/arm64", "libten_vad.dylib")
        } else {
            return Err("Unsupported macOS architecture".to_string());
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            ("Windows/x64", "ten_vad.dll")
        } else if cfg!(target_arch = "x86") {
            ("Windows/x86", "ten_vad.dll")
        } else {
            return Err("Unsupported Windows architecture".to_string());
        }
    } else {
        return Err("Unsupported operating system".to_string());
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

fn pcm_i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples
        .iter()
        .map(|&sample| sample as f32 / 32768.0)
        .collect()
}

fn vad_pcm_audio_input(samples: Vec<i16>) -> AudioInput {
    AudioInput::Samples(pcm_i16_to_f32(&samples), SAMPLE_RATE)
}

async fn transcribe_audio_internal(
    _app: AppHandle,
    audio_data: Vec<i16>,
) -> Result<String, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        let input = vad_pcm_audio_input(audio_data);
        let config = stt_core::SttConfig {
            language: None,
            ..Default::default()
        };

        let engine = crate::asr::get_stt_engine(Some(_app)).await?;
        let result = engine
            .transcribe(input, config)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result.text)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = (_app, audio_data);
        Err("STT engine not available".into())
    }
}

#[tauri::command]
pub async fn start_listening(
    app: AppHandle,
    state: tauri::State<'_, VadRecorderState>,
    config_state: tauri::State<'_, VadRuntimeConfigState>,
) -> Result<(), String> {
    let lib_path = get_vad_lib_path(&app)?;

    {
        let recorder = state.recorder.lock();
        let starting = state.starting.lock();
        if recorder.is_some() || *starting {
            return Err("Already listening".into());
        }
    }

    {
        let mut starting = state.starting.lock();
        if *starting {
            return Err("Already listening".into());
        }
        *starting = true;
    }

    let session_id = state.allocate_session_id();
    state.set_active_session(session_id);

    let runtime_config = config_state.get();
    let vad_config = VadConfig {
        threshold: runtime_config.threshold,
        ..Default::default()
    };

    let recorder = match AudioRecorder::new(&lib_path, &vad_config) {
        Ok(recorder) => recorder,
        Err(err) => {
            state.clear_active_session();
            let mut starting = state.starting.lock();
            *starting = false;
            return Err(err.to_string());
        }
    };

    let sm = recorder.state_machine();
    let event_rx = recorder.event_rx();
    let active_session = state.active_session.clone();
    let recorder_slot = state.recorder.clone();
    let starting = state.starting.clone();

    {
        let mut guard = state.recorder.lock();
        *guard = Some(recorder);
    }

    {
        let mut starting = state.starting.lock();
        *starting = false;
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = event_rx.recv() {
            match event {
                crate::vad::VadEvent::StateChanged(s) => {
                    if is_session_active(&active_session, session_id) {
                        let _ = app_clone.emit(
                            "vad-state",
                            serde_json::json!({ "state": s.to_string(), "sessionId": session_id }),
                        );
                    }
                }
                crate::vad::VadEvent::SpeechDetected(audio_data) => {
                    match transcribe_audio_internal(app_clone.clone(), audio_data).await {
                        Ok(text) => {
                            if is_session_active(&active_session, session_id) {
                                let prompt = text.clone();
                                let _ = app_clone.emit(
                                    "transcript",
                                    serde_json::json!({ "text": text, "sessionId": session_id }),
                                );
                                let runtime = app_clone.state::<crate::acp::AcpRuntime>();
                                if let Err(e) = runtime.send_prompt(app_clone.clone(), prompt).await
                                {
                                    crate::acp::session::emit_agent_event(
                                        &app_clone,
                                        crate::acp::AgentEvent::error(format!(
                                            "Failed to send current sentence: {e}"
                                        )),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            if is_session_active(&active_session, session_id) {
                                let _ = app_clone.emit(
                                    "error",
                                    serde_json::json!({ "message": e, "sessionId": session_id }),
                                );
                                crate::acp::session::emit_agent_event(
                                    &app_clone,
                                    crate::acp::AgentEvent::error("Speech transcription failed"),
                                );
                            }
                        }
                    }
                    if is_session_active(&active_session, session_id) {
                        let mut sm_guard = sm.lock();
                        sm_guard.finish_transcription();
                    }
                }
                crate::vad::VadEvent::Error(msg) => {
                    if is_session_active(&active_session, session_id) {
                        let _ = app_clone.emit(
                            "error",
                            serde_json::json!({ "message": msg, "sessionId": session_id }),
                        );
                        let mut sm_guard = sm.lock();
                        sm_guard.stop();
                        drop(sm_guard);

                        {
                            let mut recorder_guard = recorder_slot.lock();
                            recorder_guard.take();
                        }

                        {
                            let mut session_guard = active_session.lock();
                            session_guard.take();
                        }

                        {
                            let mut starting_guard = starting.lock();
                            *starting_guard = false;
                        }
                    }
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
pub fn get_vad_config(state: tauri::State<'_, VadRuntimeConfigState>) -> Result<VadRuntimeConfig, String> {
    Ok(state.get())
}

#[tauri::command]
pub fn set_vad_config(
    state: tauri::State<'_, VadRuntimeConfigState>,
    config: VadRuntimeConfig,
) -> Result<(), String> {
    validate_threshold(config.threshold)?;

    state.set(config);
    Ok(())
}

fn validate_threshold(threshold: f32) -> Result<(), String> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err("threshold must be between 0.0 and 1.0".into());
    }
    Ok(())
}

#[tauri::command]
pub fn stop_listening(
    app: AppHandle,
    state: tauri::State<'_, VadRecorderState>,
) -> Result<(), String> {
    let stopped_session = state.clear_active_session();

    let mut guard = state.recorder.lock();
    if let Some(recorder) = guard.take() {
        let sm = recorder.state_machine();
        let mut sm = sm.lock();
        sm.stop();
    }

    if let Some(session_id) = stopped_session {
        let _ = app.emit(
            "vad-state",
            serde_json::json!({ "state": VadState::Idle.to_string(), "sessionId": session_id }),
        );
    }

    {
        let mut starting = state.starting.lock();
        *starting = false;
    }

    Ok(())
}

#[tauri::command]
pub fn get_vad_state(state: tauri::State<'_, VadRecorderState>) -> Result<String, String> {
    if state.current_active_session().is_none() {
        return Ok(VadState::Idle.to_string());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_i16_to_f32_normalizes_positive_negative_and_zero_samples() {
        let samples = pcm_i16_to_f32(&[0, 16384, i16::MAX, i16::MIN]);

        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], 0.5);
        assert!((samples[2] - 32767.0 / 32768.0).abs() < f32::EPSILON);
        assert_eq!(samples[3], -1.0);
    }

    #[test]
    fn vad_pcm_audio_input_constructs_samples_variant() {
        let input = vad_pcm_audio_input(vec![0, 16384, i16::MIN]);

        match input {
            AudioInput::Samples(samples, sample_rate) => {
                assert_eq!(sample_rate, SAMPLE_RATE);
                assert_eq!(samples, vec![0.0, 0.5, -1.0]);
            }
            other => panic!("Expected Samples input, got {:?}", other),
        }
    }

    #[test]
    fn set_vad_config_rejects_out_of_range_threshold() {
        let result = validate_threshold(1.2);
        assert!(result.is_err());
        assert_eq!(result.err().as_deref(), Some("threshold must be between 0.0 and 1.0"));
    }
}
