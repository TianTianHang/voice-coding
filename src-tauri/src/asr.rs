use std::io::Write;
use std::path::Path;
#[cfg(feature = "stt-qwen3")]
use std::sync::Arc;
use stt_core::{AudioInput, SttConfig, SttEngine};
#[cfg(feature = "stt-qwen3")]
use stt_qwen3::{Qwen3AsrEngine, Qwen3LoadTiming};
#[cfg(feature = "stt-qwen3")]
use tauri::{AppHandle, Emitter};
#[cfg(feature = "stt-qwen3")]
use tokio::sync::{Mutex, Notify};

#[cfg(feature = "stt-qwen3")]
use crate::model_paths::{
    ModelPathSnapshot, ResolvedModelPath, resolve_asr_model_path, resolve_asr_model_path_with_app,
};

#[cfg(feature = "stt-qwen3")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AsrLoadState {
    Unloaded,
    Loading,
    Ready,
    Failed,
}

#[cfg(feature = "stt-qwen3")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStatusSnapshot {
    pub state: AsrLoadState,
    pub engine_name: String,
    pub model_dir: String,
    pub model: ModelPathSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<Qwen3LoadTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(feature = "stt-qwen3")]
type EngineLoader = dyn Fn(&str) -> Result<Qwen3AsrEngine, String> + Send + Sync;

#[cfg(feature = "stt-qwen3")]
struct AsrRuntimeInner {
    snapshot: AsrStatusSnapshot,
    engine: Option<Arc<Qwen3AsrEngine>>,
    loading: Option<Arc<Notify>>,
}

#[cfg(feature = "stt-qwen3")]
pub struct AsrRuntime {
    inner: Mutex<AsrRuntimeInner>,
    loader: Arc<EngineLoader>,
}

#[cfg(feature = "stt-qwen3")]
impl AsrStatusSnapshot {
    fn unloaded(model_path: ResolvedModelPath) -> Self {
        let model_dir = model_path.engine_model_dir_string();
        Self {
            state: AsrLoadState::Unloaded,
            engine_name: model_path.engine_name.to_string(),
            model: model_path.snapshot(),
            model_dir,
            phase: None,
            timing: None,
            error: None,
        }
    }

    fn loading(model_path: ResolvedModelPath) -> Self {
        let model_dir = model_path.engine_model_dir_string();
        Self {
            state: AsrLoadState::Loading,
            engine_name: model_path.engine_name.to_string(),
            model: model_path.snapshot(),
            model_dir,
            phase: Some("model".to_string()),
            timing: None,
            error: None,
        }
    }

    fn ready(model_path: ResolvedModelPath, timing: Qwen3LoadTiming) -> Self {
        let model_dir = model_path.engine_model_dir_string();
        Self {
            state: AsrLoadState::Ready,
            engine_name: model_path.engine_name.to_string(),
            model: model_path.snapshot(),
            model_dir,
            phase: None,
            timing: Some(timing),
            error: None,
        }
    }

    fn failed(model_path: ResolvedModelPath, error: String) -> Self {
        let model_dir = model_path.engine_model_dir_string();
        let mut model = model_path.snapshot();
        model.error = Some(error.clone());
        Self {
            state: AsrLoadState::Failed,
            engine_name: model_path.engine_name.to_string(),
            model,
            model_dir,
            phase: None,
            timing: None,
            error: Some(error),
        }
    }
}

#[cfg(feature = "stt-qwen3")]
impl AsrRuntime {
    pub fn new() -> Self {
        Self::new_with_loader(|model_dir| Qwen3AsrEngine::new(model_dir).map_err(|e| e.to_string()))
    }

    fn new_with_loader(
        loader: impl Fn(&str) -> Result<Qwen3AsrEngine, String> + Send + Sync + 'static,
    ) -> Self {
        let model_path = resolve_asr_model_path();
        Self {
            inner: Mutex::new(AsrRuntimeInner {
                snapshot: AsrStatusSnapshot::unloaded(model_path),
                engine: None,
                loading: None,
            }),
            loader: Arc::new(loader),
        }
    }

    pub async fn status(&self) -> AsrStatusSnapshot {
        self.inner.lock().await.snapshot.clone()
    }

    pub async fn prepare(&self, app: Option<AppHandle>) -> AsrStatusSnapshot {
        loop {
            let load = {
                let mut inner = self.inner.lock().await;
                match inner.snapshot.state {
                    AsrLoadState::Ready => return inner.snapshot.clone(),
                    AsrLoadState::Loading => {
                        let notify = inner
                            .loading
                            .as_ref()
                            .expect("loading notify exists")
                            .clone();
                        LoadAction::Wait(notify)
                    }
                    AsrLoadState::Unloaded | AsrLoadState::Failed => {
                        let notify = Arc::new(Notify::new());
                        let model_path = app
                            .as_ref()
                            .map(resolve_asr_model_path_with_app)
                            .unwrap_or_else(resolve_asr_model_path);
                        let model_dir = model_path.engine_model_dir_string();
                        inner.engine = None;
                        inner.loading = Some(notify.clone());
                        inner.snapshot = AsrStatusSnapshot::loading(model_path.clone());
                        let snapshot = inner.snapshot.clone();
                        drop(inner);
                        emit_asr_status(app.as_ref(), &snapshot);
                        LoadAction::Start {
                            notify,
                            model_path,
                            model_dir,
                        }
                    }
                }
            };

            match load {
                LoadAction::Wait(notify) => notify.notified().await,
                LoadAction::Start {
                    notify,
                    model_path,
                    model_dir,
                } => {
                    let loader = self.loader.clone();
                    let load_model_dir = model_dir.clone();
                    let load_result =
                        tauri::async_runtime::spawn_blocking(move || loader(&load_model_dir))
                            .await
                            .map_err(|e| format!("ASR loader task failed: {}", e))
                            .and_then(|result| result);

                    let snapshot = {
                        let mut inner = self.inner.lock().await;
                        match load_result {
                            Ok(engine) => {
                                let timing = engine.load_timing();
                                inner.engine = Some(Arc::new(engine));
                                inner.snapshot = AsrStatusSnapshot::ready(model_path, timing);
                            }
                            Err(error) => {
                                inner.engine = None;
                                inner.snapshot = AsrStatusSnapshot::failed(model_path, error);
                            }
                        }
                        inner.loading = None;
                        let snapshot = inner.snapshot.clone();
                        notify.notify_waiters();
                        snapshot
                    };

                    emit_asr_status(app.as_ref(), &snapshot);
                    return snapshot;
                }
            }
        }
    }

    pub async fn ready_engine(
        &self,
        app: Option<AppHandle>,
    ) -> Result<Arc<Qwen3AsrEngine>, String> {
        loop {
            let wait = {
                let inner = self.inner.lock().await;
                match inner.snapshot.state {
                    AsrLoadState::Ready => {
                        return inner.engine.clone().ok_or_else(|| {
                            "ASR runtime is ready but no engine is available".to_string()
                        });
                    }
                    AsrLoadState::Loading => Some(
                        inner
                            .loading
                            .as_ref()
                            .expect("loading notify exists")
                            .clone(),
                    ),
                    AsrLoadState::Failed => {
                        return Err(format!(
                            "ASR model failed to load: {}",
                            inner
                                .snapshot
                                .error
                                .clone()
                                .unwrap_or_else(|| "unknown error".to_string())
                        ));
                    }
                    AsrLoadState::Unloaded => None,
                }
            };

            if let Some(notify) = wait {
                notify.notified().await;
            } else {
                let snapshot = self.prepare(app.clone()).await;
                if snapshot.state == AsrLoadState::Failed {
                    return Err(format!(
                        "ASR model failed to load: {}",
                        snapshot
                            .error
                            .unwrap_or_else(|| "unknown error".to_string())
                    ));
                }
            }
        }
    }
}

#[cfg(feature = "stt-qwen3")]
enum LoadAction {
    Wait(Arc<Notify>),
    Start {
        notify: Arc<Notify>,
        model_path: ResolvedModelPath,
        model_dir: String,
    },
}

#[cfg(feature = "stt-qwen3")]
static ASR_RUNTIME: once_cell::sync::Lazy<AsrRuntime> = once_cell::sync::Lazy::new(AsrRuntime::new);

#[cfg(feature = "stt-qwen3")]
fn emit_asr_status(app: Option<&AppHandle>, snapshot: &AsrStatusSnapshot) {
    if let Some(app) = app {
        let _ = app.emit("asr-status", snapshot);
    }
}

#[cfg(feature = "stt-qwen3")]
pub async fn get_stt_engine(app: Option<AppHandle>) -> Result<Arc<Qwen3AsrEngine>, String> {
    ASR_RUNTIME.ready_engine(app).await
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
pub async fn prepare_asr(app: tauri::AppHandle) -> Result<AsrStatusSnapshot, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        Ok(ASR_RUNTIME.prepare(Some(app)).await)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        let _ = app;
        Err("STT engine not available: no engine feature enabled".into())
    }
}

#[tauri::command]
pub async fn get_asr_status() -> Result<AsrStatusSnapshot, String> {
    #[cfg(feature = "stt-qwen3")]
    {
        Ok(ASR_RUNTIME.status().await)
    }

    #[cfg(not(feature = "stt-qwen3"))]
    {
        Err("STT engine not available: no engine feature enabled".into())
    }
}

#[cfg(feature = "stt-qwen3")]
pub fn prewarm_asr(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = ASR_RUNTIME.prepare(Some(app)).await;
    });
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

        let engine = get_stt_engine(None).await?;
        let result = engine
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

        let engine = get_stt_engine(None).await;
        let transcription_result = match engine {
            Ok(engine) => engine
                .transcribe(input, config)
                .await
                .map(|result| result.text)
                .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };

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
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cleanup_temp_audio_file_removes_existing_file() {
        let temp_file =
            std::env::temp_dir().join(format!("voice-coding-test-{}.wav", uuid::Uuid::new_v4()));
        std::fs::write(&temp_file, b"wav").expect("failed to create temp file");

        let result = cleanup_temp_audio_file(&temp_file);
        assert!(result.is_ok());
        assert!(!temp_file.exists());
    }

    #[test]
    fn cleanup_temp_audio_file_ignores_missing_file() {
        let temp_file = std::env::temp_dir().join(format!(
            "voice-coding-test-missing-{}.wav",
            uuid::Uuid::new_v4()
        ));

        let result = cleanup_temp_audio_file(&temp_file);
        assert!(result.is_ok());
    }

    #[cfg(feature = "stt-qwen3")]
    #[tokio::test]
    async fn runtime_starts_unloaded() {
        let runtime = AsrRuntime::new_with_loader(|_| Err("not used".to_string()));

        let status = runtime.status().await;

        assert_eq!(status.state, AsrLoadState::Unloaded);
        assert_eq!(status.engine_name, status.model.engine_name);
        assert_eq!(status.model_dir, status.model.model_dir);
        assert!(status.error.is_none());
    }

    #[cfg(feature = "stt-qwen3")]
    #[tokio::test]
    async fn runtime_failed_status_can_retry_loader() {
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = calls.clone();
        let runtime = AsrRuntime::new_with_loader(move |_| {
            loader_calls.fetch_add(1, Ordering::SeqCst);
            Err("missing model".to_string())
        });

        let first = runtime.prepare(None).await;
        let second = runtime.prepare(None).await;

        assert_eq!(first.state, AsrLoadState::Failed);
        assert_eq!(second.state, AsrLoadState::Failed);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(second.error.as_deref(), Some("missing model"));
        assert_eq!(second.model.error.as_deref(), Some("missing model"));
    }

    #[cfg(feature = "stt-qwen3")]
    #[tokio::test]
    async fn ready_engine_returns_failed_state_error() {
        let runtime = AsrRuntime::new_with_loader(|_| Err("bad load".to_string()));

        let status = runtime.prepare(None).await;
        let result = runtime.ready_engine(None).await;

        assert_eq!(status.state, AsrLoadState::Failed);
        assert_eq!(
            result
                .err()
                .expect("failed runtime should reject transcription"),
            "ASR model failed to load: bad load"
        );
    }

    #[cfg(feature = "stt-qwen3")]
    #[tokio::test]
    async fn prepare_reuses_inflight_loader_while_loading() {
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let loader_calls = calls.clone();
        let loader_release = release.clone();

        let runtime = Arc::new(AsrRuntime::new_with_loader(move |_| {
            loader_calls.fetch_add(1, Ordering::SeqCst);
            while !loader_release.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err("load failed once".to_string())
        }));

        let runtime_one = runtime.clone();
        let first = tokio::spawn(async move { runtime_one.prepare(None).await });

        let mut seen_loading = false;
        for _ in 0..1000 {
            if calls.load(Ordering::SeqCst) == 1
                && runtime.status().await.state == AsrLoadState::Loading
            {
                seen_loading = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        assert!(seen_loading);

        let runtime_two = runtime.clone();
        let second = tokio::spawn(async move { runtime_two.prepare(None).await });

        for _ in 0..50 {
            if calls.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        release.store(true, Ordering::SeqCst);

        let first_status = first.await.expect("first prepare task panicked");
        let second_status = second.await.expect("second prepare task panicked");

        assert_eq!(first_status.state, AsrLoadState::Failed);
        assert_eq!(second_status.state, AsrLoadState::Failed);
        assert_eq!(first_status.error.as_deref(), Some("load failed once"));
        assert_eq!(second_status.error.as_deref(), Some("load failed once"));
        assert!(calls.load(Ordering::SeqCst) >= 1);
    }

    #[cfg(feature = "stt-qwen3")]
    #[tokio::test]
    async fn ready_engine_waits_for_loading_and_uses_same_result() {
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let loader_calls = calls.clone();
        let loader_release = release.clone();

        let runtime = Arc::new(AsrRuntime::new_with_loader(move |_| {
            loader_calls.fetch_add(1, Ordering::SeqCst);
            while !loader_release.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err("load blocked failure".to_string())
        }));

        let prepare_runtime = runtime.clone();
        let prepare_task = tokio::spawn(async move { prepare_runtime.prepare(None).await });

        let mut seen_loading = false;
        for _ in 0..1000 {
            if calls.load(Ordering::SeqCst) == 1
                && runtime.status().await.state == AsrLoadState::Loading
            {
                seen_loading = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        assert!(seen_loading);

        let ready_runtime = runtime.clone();
        let ready_task = tokio::spawn(async move { ready_runtime.ready_engine(None).await });

        for _ in 0..50 {
            if calls.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        release.store(true, Ordering::SeqCst);

        let status = prepare_task.await.expect("prepare task panicked");
        let ready_result = ready_task.await.expect("ready_engine task panicked");

        assert_eq!(status.state, AsrLoadState::Failed);
        match ready_result {
            Ok(_) => panic!("ready_engine should fail after loader failure"),
            Err(err) => assert_eq!(err, "ASR model failed to load: load blocked failure"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
