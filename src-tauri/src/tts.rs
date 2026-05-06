use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Duration};
#[cfg(any(test, feature = "tts-mock"))]
use tts_core::{AudioBuffer, PcmData, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ};
use tts_core::{TtsConfig, TtsEngine, TtsError, TtsResult};

use crate::audio::{playback_buffer_from_tts, AudioOutput};
use crate::model_paths::{resolve_tts_model_path, resolve_tts_model_path_with_app, ModelPathSnapshot};
#[cfg(feature = "tts-moss-onnx")]
use tts_moss::{MossModelConfig, MossOnnxTtsEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsState {
    Idle,
    Synthesizing,
    Ready,
    Playing,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatusSnapshot {
    pub state: TtsState,
    pub engine_name: String,
    pub model: ModelPathSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub has_buffered_audio: bool,
}

struct TtsRuntimeInner {
    snapshot: TtsStatusSnapshot,
    latest_result: Option<TtsResult>,
    output: Option<AudioOutput>,
    paused_recording: bool,
    cancel_requested: bool,
}

pub struct TtsRuntime {
    inner: Arc<Mutex<TtsRuntimeInner>>,
    engine: Arc<dyn TtsEngine>,
    engine_name: String,
    model: ModelPathSnapshot,
}

#[cfg(feature = "tts-moss-onnx")]
fn default_tts_model_snapshot() -> ModelPathSnapshot {
    resolve_tts_model_path().snapshot()
}

#[cfg(not(feature = "tts-moss-onnx"))]
fn default_tts_model_snapshot() -> ModelPathSnapshot {
    ModelPathSnapshot {
        kind: crate::model_paths::ModelKind::Tts,
        model_id: crate::model_paths::MOSS_TTS_MODEL_ID.to_string(),
        engine_name: crate::model_paths::MOSS_TTS_ENGINE_NAME.to_string(),
        package_dir: String::new(),
        model_dir: String::new(),
        source: crate::model_paths::ModelPathSource::DevFallback,
        legacy_layout: false,
        missing_files: Vec::new(),
        error: Some("TTS model path resolution is unavailable without MOSS ONNX TTS".to_string()),
    }
}

#[cfg(feature = "tts-moss-onnx")]
fn tts_model_snapshot_with_app(app: &AppHandle) -> ModelPathSnapshot {
    resolve_tts_model_path_with_app(app).snapshot()
}

#[cfg(not(feature = "tts-moss-onnx"))]
fn tts_model_snapshot_with_app(_app: &AppHandle) -> ModelPathSnapshot {
    default_tts_model_snapshot()
}

impl Default for TtsRuntime {
    fn default() -> Self {
        Self::new(default_tts_engine())
    }
}

fn default_tts_engine() -> Arc<dyn TtsEngine> {
    #[cfg(feature = "tts-moss-onnx")]
    {
        let model_path = resolve_tts_model_path();
        let config = MossModelConfig {
            model_dir: model_path.engine_model_dir,
        };
        match MossOnnxTtsEngine::new(config) {
            Ok(engine) => Arc::new(engine),
            Err(err) => Arc::new(StartupErrorTtsEngine::new("moss-onnx-tts", err.to_string())),
        }
    }

    #[cfg(all(not(feature = "tts-moss-onnx"), feature = "tts-mock"))]
    {
        Arc::new(MockTtsEngine)
    }

    #[cfg(all(not(feature = "tts-moss-onnx"), not(feature = "tts-mock")))]
    {
        Arc::new(StartupErrorTtsEngine::new(
            "unavailable-tts",
            "TTS engine not available: no engine feature enabled".to_string(),
        ))
    }
}

impl TtsRuntime {
    pub fn new(engine: Arc<dyn TtsEngine>) -> Self {
        Self::new_with_model(engine, default_tts_model_snapshot())
    }

    pub fn with_app(app: &AppHandle) -> Self {
        let model = tts_model_snapshot_with_app(app);
        let engine = default_tts_engine_with_model_dir(std::path::PathBuf::from(&model.model_dir));
        Self::new_with_model(engine, model)
    }

    fn new_with_model(engine: Arc<dyn TtsEngine>, model: ModelPathSnapshot) -> Self {
        let engine_name = engine.engine_name().to_string();
        Self {
            inner: Arc::new(Mutex::new(TtsRuntimeInner {
                snapshot: TtsStatusSnapshot {
                    state: TtsState::Idle,
                    engine_name: engine_name.clone(),
                    model: model.clone(),
                    error: None,
                    has_buffered_audio: false,
                },
                latest_result: None,
                output: None,
                paused_recording: false,
                cancel_requested: false,
            })),
            engine,
            engine_name,
            model,
        }
    }

    fn set_state(&self, app: Option<&AppHandle>, state: TtsState, error: Option<String>) {
        let snapshot = {
            let mut inner = self.inner.lock();
            inner.snapshot.state = state;
            inner.snapshot.engine_name = self.engine_name.clone();
            inner.snapshot.model = self.model.clone();
            inner.snapshot.error = error;
            inner.snapshot.has_buffered_audio = inner.latest_result.is_some();
            inner.snapshot.clone()
        };
        if let Some(app) = app {
            let _ = app.emit("tts-state", &snapshot);
        }
    }

    pub fn status(&self) -> TtsStatusSnapshot {
        self.inner.lock().snapshot.clone()
    }

    pub async fn prepare(&self, app: Option<&AppHandle>) -> Result<TtsStatusSnapshot, String> {
        let healthy = self.engine.health_check().await.map_err(|e| e.to_string())?;
        if healthy {
            self.set_state(app, TtsState::Idle, None);
            Ok(self.status())
        } else {
            self.set_state(app, TtsState::Failed, Some("TTS engine health check failed".to_string()));
            Err("TTS engine health check failed".to_string())
        }
    }

    pub async fn synthesize(
        &self,
        app: Option<&AppHandle>,
        text: String,
        config: TtsConfig,
    ) -> Result<TtsStatusSnapshot, String> {
        self.set_state(app, TtsState::Synthesizing, None);

        let result = self
            .engine
            .synthesize(&text, config)
            .await
            .map_err(|e| e.to_string())?;
        result.validate_for_playback().map_err(|e| e.to_string())?;

        {
            let mut inner = self.inner.lock();
            inner.latest_result = Some(result);
            inner.snapshot.has_buffered_audio = true;
        }

        self.set_state(app, TtsState::Ready, None);
        Ok(self.status())
    }

    pub fn cancel_playback(&self) {
        self.inner.lock().cancel_requested = true;
    }
}

#[cfg(feature = "tts-moss-onnx")]
fn default_tts_engine_with_model_dir(model_dir: std::path::PathBuf) -> Arc<dyn TtsEngine> {
    let config = MossModelConfig { model_dir };
    match MossOnnxTtsEngine::new(config) {
        Ok(engine) => Arc::new(engine),
        Err(err) => Arc::new(StartupErrorTtsEngine::new("moss-onnx-tts", err.to_string())),
    }
}

#[cfg(not(feature = "tts-moss-onnx"))]
fn default_tts_engine_with_model_dir(_model_dir: std::path::PathBuf) -> Arc<dyn TtsEngine> {
    default_tts_engine()
}

#[cfg(any(test, feature = "tts-mock"))]
struct MockTtsEngine;

#[cfg(any(
    feature = "tts-moss-onnx",
    all(not(feature = "tts-moss-onnx"), not(feature = "tts-mock"))
))]
struct StartupErrorTtsEngine {
    name: &'static str,
    error: String,
}

#[cfg(any(
    feature = "tts-moss-onnx",
    all(not(feature = "tts-moss-onnx"), not(feature = "tts-mock"))
))]
impl StartupErrorTtsEngine {
    fn new(name: &'static str, error: String) -> Self {
        Self { name, error }
    }
}

#[cfg(any(
    feature = "tts-moss-onnx",
    all(not(feature = "tts-moss-onnx"), not(feature = "tts-mock"))
))]
#[async_trait::async_trait]
impl TtsEngine for StartupErrorTtsEngine {
    fn engine_name(&self) -> &str {
        self.name
    }

    async fn synthesize(&self, _text: &str, _config: TtsConfig) -> tts_core::Result<TtsResult> {
        Err(TtsError::UnsupportedConfig(self.error.clone()))
    }

    async fn health_check(&self) -> tts_core::Result<bool> {
        Err(TtsError::UnsupportedConfig(self.error.clone()))
    }
}

#[cfg(any(test, feature = "tts-mock"))]
#[async_trait::async_trait]
impl TtsEngine for MockTtsEngine {
    fn engine_name(&self) -> &str {
        "mock-tts"
    }

    async fn synthesize(&self, text: &str, _config: TtsConfig) -> tts_core::Result<TtsResult> {
        if text.trim().is_empty() {
            return Err(TtsError::InvalidInput("text must not be empty".to_string()));
        }

        let frame_count = (PLAYBACK_SAMPLE_RATE_HZ as usize / 6).max(text.len() * 128);
        let mut samples = Vec::with_capacity(frame_count * PLAYBACK_CHANNELS as usize);
        for i in 0..frame_count {
            let t = i as f32 / PLAYBACK_SAMPLE_RATE_HZ as f32;
            let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.08;
            samples.push(sample);
            samples.push(sample);
        }

        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(samples),
            },
        })
    }

    async fn health_check(&self) -> tts_core::Result<bool> {
        Ok(true)
    }
}

#[tauri::command]
pub async fn prepare_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
) -> Result<TtsStatusSnapshot, String> {
    runtime.prepare(Some(&app)).await
}

#[tauri::command]
pub fn get_tts_status(runtime: tauri::State<'_, TtsRuntime>) -> Result<TtsStatusSnapshot, String> {
    Ok(runtime.status())
}

#[tauri::command]
pub async fn synthesize_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    text: String,
) -> Result<TtsStatusSnapshot, String> {
    runtime.synthesize(Some(&app), text, TtsConfig::default()).await
}

#[tauri::command]
pub async fn play_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
) -> Result<TtsStatusSnapshot, String> {
    let buffer = {
        let mut inner = runtime.inner.lock();
        inner.cancel_requested = false;
        let result = inner
            .latest_result
            .as_ref()
            .ok_or_else(|| "No synthesized audio available".to_string())?;
        playback_buffer_from_tts(&result.audio).map_err(|e| e.to_string())?
    };

    let had_active_session = vad_state.has_active_session();
    if had_active_session {
        crate::vad_commands::stop_listening(app.clone(), vad_state.clone())?;
        runtime.inner.lock().paused_recording = true;
    }

    runtime.set_state(Some(&app), TtsState::Playing, None);

    let output = match AudioOutput::new() {
        Ok(output) => output,
        Err(e) => {
            runtime.set_state(Some(&app), TtsState::Failed, Some(e.to_string()));
            if runtime.inner.lock().paused_recording {
                let _ = crate::vad_commands::start_listening(
                    app.clone(),
                    vad_state.clone(),
                    vad_config_state,
                )
                .await;
                runtime.inner.lock().paused_recording = false;
            }
            return Err("Failed to create output stream".to_string());
        }
    };

    output.enqueue(buffer);
    {
        let mut inner = runtime.inner.lock();
        inner.output = Some(output);
    }

    loop {
        let (cancelled, done) = {
            let inner = runtime.inner.lock();
            let done = inner.output.as_ref().map(|o| o.is_empty()).unwrap_or(true);
            (inner.cancel_requested, done)
        };

        if cancelled {
            if let Some(output) = runtime.inner.lock().output.as_ref() {
                output.clear();
            }
            break;
        }

        if done {
            break;
        }

        sleep(Duration::from_millis(20)).await;
    }

    {
        let mut inner = runtime.inner.lock();
        inner.output.take();
    }

    if runtime.inner.lock().paused_recording {
        crate::vad_commands::start_listening(app.clone(), vad_state, vad_config_state).await?;
        runtime.inner.lock().paused_recording = false;
    }

    runtime.set_state(Some(&app), TtsState::Idle, None);
    Ok(runtime.status())
}

#[tauri::command]
pub async fn cancel_tts_playback(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
) -> Result<TtsStatusSnapshot, String> {
    runtime.cancel_playback();

    let should_resume = runtime.inner.lock().paused_recording;
    if should_resume {
        let _ = crate::vad_commands::start_listening(app.clone(), vad_state, vad_config_state).await;
        runtime.inner.lock().paused_recording = false;
    }

    runtime.set_state(Some(&app), TtsState::Idle, None);
    Ok(runtime.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingEngine;

    #[async_trait::async_trait]
    impl TtsEngine for FailingEngine {
        fn engine_name(&self) -> &str {
            "fail"
        }

        async fn synthesize(&self, _text: &str, _config: TtsConfig) -> tts_core::Result<TtsResult> {
            Err(TtsError::SynthesisError("boom".to_string()))
        }

        async fn health_check(&self) -> tts_core::Result<bool> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn runtime_transitions_to_ready_after_synthesis() {
        let runtime = TtsRuntime::new(Arc::new(MockTtsEngine));

        let _ = runtime
            .synthesize(None, "hello".to_string(), TtsConfig::default())
            .await
            .expect("synthesis should work");

        let status = runtime.status();
        assert_eq!(status.state, TtsState::Ready);
        assert_eq!(status.engine_name, "mock-tts");
        assert_eq!(status.model.kind, crate::model_paths::ModelKind::Tts);
        assert!(status.has_buffered_audio);
    }

    #[tokio::test]
    async fn failed_health_check_marks_runtime_failed() {
        let runtime = TtsRuntime::new(Arc::new(FailingEngine));

        let result = runtime.prepare(None).await;
        assert!(result.is_err());

        let status = runtime.status();
        assert_eq!(status.state, TtsState::Failed);
        assert_eq!(status.engine_name, "fail");
    }

    #[tokio::test]
    async fn status_events_are_complete_snapshots() {
        let model = crate::model_paths::ResolvedModelPath {
            kind: crate::model_paths::ModelKind::Tts,
            model_id: crate::model_paths::MOSS_TTS_MODEL_ID,
            engine_name: crate::model_paths::MOSS_TTS_ENGINE_NAME,
            package_dir: std::path::PathBuf::from("models/tts/moss-tts-nano-100m-onnx"),
            engine_model_dir: std::path::PathBuf::from("models/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX"),
            source: crate::model_paths::ModelPathSource::DevFallback,
            legacy_layout: false,
            missing_files: Vec::new(),
            error: None,
        }
        .snapshot();
        let runtime = TtsRuntime::new_with_model(Arc::new(MockTtsEngine), model.clone());

        runtime.set_state(None, TtsState::Synthesizing, None);
        let status = runtime.status();

        assert_eq!(status.state, TtsState::Synthesizing);
        assert_eq!(status.engine_name, "mock-tts");
        assert_eq!(status.model, model);
    }

    #[tokio::test]
    async fn synthesis_error_propagates() {
        let runtime = TtsRuntime::new(Arc::new(FailingEngine));

        let result = runtime
            .synthesize(None, "hello".to_string(), TtsConfig::default())
            .await;
        assert!(result.is_err());
    }
}
