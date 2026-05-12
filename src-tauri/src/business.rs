use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::acp::events::{current_millis, AgentStatus};
use crate::tts::{AutoTtsLastStatus, TtsState, TtsStatusSnapshot};
use crate::vad::VadState;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    pub voice: VoiceSessionConfig,
    pub speech: SpeechPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechPreferences {
    pub auto_speak_agent_results: bool,
}

impl Default for SpeechPreferences {
    fn default() -> Self {
        Self {
            auto_speak_agent_results: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppReadiness {
    Initializing,
    Ready,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub readiness: AppReadiness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub asr: serde_json::Value,
    pub tts: TtsStatusSnapshot,
    pub voice: VoiceSessionStatus,
    pub agent: AgentStatus,
    pub speech: SpeechOutputStatus,
    pub preferences: AppPreferences,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceInputMode {
    DictationOnly,
    AutoSendToAgent,
    ConfirmBeforeSend,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSessionConfig {
    pub input_mode: VoiceInputMode,
}

impl Default for VoiceSessionConfig {
    fn default() -> Self {
        Self {
            input_mode: VoiceInputMode::AutoSendToAgent,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSessionState {
    Idle,
    Starting,
    Listening,
    Recording,
    Transcribing,
    Paused,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoicePauseReason {
    TtsPlayback,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSessionStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<u64>,
    pub state: VoiceSessionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_reason: Option<VoicePauseReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub config: VoiceSessionConfig,
}

impl VoiceSessionStatus {
    fn idle(config: VoiceSessionConfig) -> Self {
        Self {
            session_id: None,
            state: VoiceSessionState::Idle,
            pause_reason: None,
            error: None,
            config,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceUtteranceKind {
    Detected,
    Transcribed,
    SubmittedToAgent,
    Discarded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UtteranceStatus {
    PendingConfirmation,
    SubmittedToAgent,
    Discarded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceUtteranceEvent {
    pub kind: VoiceUtteranceKind,
    pub session_id: u64,
    pub utterance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_transcript: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone)]
struct VoiceUtteranceRecord {
    session_id: u64,
    transcript: String,
    original_transcript: String,
    status: UtteranceStatus,
    turn_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentMessageSource {
    Manual,
    Voice,
    EditedTranscript,
    Retry,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendAgentMessageRequest {
    pub text: String,
    pub source: AgentMessageSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utterance_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentTurnState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnStatus {
    pub turn_id: String,
    pub state: AgentTurnState,
    pub source: AgentMessageSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utterance_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechOutputState {
    Idle,
    Synthesizing,
    Ready,
    Playing,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechOutputSource {
    Text,
    AgentResult,
    AutoAgentResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeechOutputStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_id: Option<String>,
    pub state: SpeechOutputState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SpeechOutputSource>,
    pub auto_speak_agent_results: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SpeechOutputStatus {
    fn idle(auto_speak_agent_results: bool) -> Self {
        Self {
            speech_id: None,
            state: SpeechOutputState::Idle,
            source: None,
            auto_speak_agent_results,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeErrorEvent {
    pub scope: String,
    pub message: String,
    pub recoverable: bool,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakTextRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakAgentResultRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
}

#[derive(Debug, Clone)]
struct BusinessRuntimeInner {
    app_readiness: AppReadiness,
    app_error: Option<String>,
    voice: VoiceSessionStatus,
    speech: SpeechOutputStatus,
    preferences: AppPreferences,
    utterances: HashMap<String, VoiceUtteranceRecord>,
    turns: HashMap<String, AgentTurnStatus>,
    active_turn_id: Option<String>,
}

pub struct BusinessRuntime {
    inner: Arc<Mutex<BusinessRuntimeInner>>,
    next_utterance_id: AtomicU64,
    next_turn_id: AtomicU64,
    next_speech_id: AtomicU64,
}

impl Default for BusinessRuntime {
    fn default() -> Self {
        let preferences = AppPreferences::default();
        Self {
            inner: Arc::new(Mutex::new(BusinessRuntimeInner {
                app_readiness: AppReadiness::Initializing,
                app_error: None,
                voice: VoiceSessionStatus::idle(preferences.voice.clone()),
                speech: SpeechOutputStatus::idle(preferences.speech.auto_speak_agent_results),
                preferences,
                utterances: HashMap::new(),
                turns: HashMap::new(),
                active_turn_id: None,
            })),
            next_utterance_id: AtomicU64::new(1),
            next_turn_id: AtomicU64::new(1),
            next_speech_id: AtomicU64::new(1),
        }
    }
}

impl BusinessRuntime {
    pub fn voice_status(&self) -> VoiceSessionStatus {
        self.inner.lock().voice.clone()
    }

    pub fn preferences(&self) -> AppPreferences {
        self.inner.lock().preferences.clone()
    }

    pub fn set_preferences(&self, preferences: AppPreferences) -> AppPreferences {
        let mut inner = self.inner.lock();
        inner.preferences = preferences.clone();
        inner.voice.config = preferences.voice.clone();
        inner.speech.auto_speak_agent_results = preferences.speech.auto_speak_agent_results;
        preferences
    }

    pub fn set_app_readiness(
        &self,
        app: &AppHandle,
        readiness: AppReadiness,
        error: Option<String>,
    ) {
        {
            let mut inner = self.inner.lock();
            inner.app_readiness = readiness;
            inner.app_error = error;
        }
        emit_app_status_changed(app);
    }

    pub fn set_voice_status(
        &self,
        app: &AppHandle,
        session_id: Option<u64>,
        state: VoiceSessionState,
        pause_reason: Option<VoicePauseReason>,
        error: Option<String>,
    ) -> VoiceSessionStatus {
        let status = {
            let mut inner = self.inner.lock();
            inner.voice.session_id = session_id;
            inner.voice.state = state;
            inner.voice.pause_reason = pause_reason;
            inner.voice.error = error;
            inner.voice.clone()
        };
        emit_voice_session_changed(app, &status);
        status
    }

    pub fn update_voice_from_vad(
        &self,
        app: &AppHandle,
        session_id: u64,
        vad_state: VadState,
    ) -> VoiceSessionStatus {
        let state = match vad_state {
            VadState::Idle => VoiceSessionState::Idle,
            VadState::Listening => VoiceSessionState::Listening,
            VadState::Recording => VoiceSessionState::Recording,
            VadState::Processing => VoiceSessionState::Transcribing,
        };
        let effective_session_id = if state == VoiceSessionState::Idle {
            None
        } else {
            Some(session_id)
        };
        self.set_voice_status(app, effective_session_id, state, None, None)
    }

    pub async fn handle_transcribed_utterance(
        &self,
        app: AppHandle,
        session_id: u64,
        transcript: String,
    ) -> Result<Option<AgentTurnStatus>, String> {
        let transcript = transcript.trim().to_string();
        if transcript.is_empty() {
            return Ok(None);
        }

        let utterance_id = self.next_utterance_id();
        let mode = {
            let mut inner = self.inner.lock();
            inner.utterances.insert(
                utterance_id.clone(),
                VoiceUtteranceRecord {
                    session_id,
                    transcript: transcript.clone(),
                    original_transcript: transcript.clone(),
                    status: UtteranceStatus::PendingConfirmation,
                    turn_id: None,
                },
            );
            inner.voice.session_id = Some(session_id);
            inner.voice.state = VoiceSessionState::Listening;
            inner.preferences.voice.input_mode
        };

        emit_voice_utterance(
            &app,
            VoiceUtteranceEvent {
                kind: VoiceUtteranceKind::Transcribed,
                session_id,
                utterance_id: utterance_id.clone(),
                transcript: Some(transcript.clone()),
                original_transcript: None,
                turn_id: None,
                error: None,
                created_at: current_millis(),
            },
        );

        match mode {
            VoiceInputMode::DictationOnly | VoiceInputMode::ConfirmBeforeSend => Ok(None),
            VoiceInputMode::AutoSendToAgent => {
                let request = SendAgentMessageRequest {
                    text: transcript,
                    source: AgentMessageSource::Voice,
                    utterance_id: Some(utterance_id.clone()),
                };
                let turn = self
                    .send_agent_message_internal(app.clone(), request)
                    .await?;
                self.mark_utterance_submitted(&app, &utterance_id, turn.turn_id.clone())?;
                Ok(Some(turn))
            }
        }
    }

    fn next_utterance_id(&self) -> String {
        format!(
            "utt-{}",
            self.next_utterance_id.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn next_turn_id(&self) -> String {
        format!("turn-{}", self.next_turn_id.fetch_add(1, Ordering::Relaxed))
    }

    fn next_speech_id(&self) -> String {
        format!(
            "speech-{}",
            self.next_speech_id.fetch_add(1, Ordering::Relaxed)
        )
    }

    async fn send_agent_message_internal(
        &self,
        app: AppHandle,
        request: SendAgentMessageRequest,
    ) -> Result<AgentTurnStatus, String> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Err("message text must not be empty".to_string());
        }

        let turn_id = self.next_turn_id();
        let now = current_millis();
        let running = AgentTurnStatus {
            turn_id: turn_id.clone(),
            state: AgentTurnState::Running,
            source: request.source,
            utterance_id: request.utterance_id.clone(),
            created_at: now,
            updated_at: now,
            error: None,
        };
        {
            let mut inner = self.inner.lock();
            inner.turns.insert(turn_id.clone(), running.clone());
            inner.active_turn_id = Some(turn_id.clone());
        }
        emit_agent_turn_changed(&app, &running);

        let runtime = app.state::<crate::acp::AcpRuntime>();
        let result = runtime.send_prompt(app.clone(), text).await;
        let completed = {
            let mut inner = self.inner.lock();
            let turn = inner
                .turns
                .get_mut(&turn_id)
                .expect("turn inserted before sending");
            if turn.state != AgentTurnState::Cancelled {
                match result {
                    Ok(()) => {
                        turn.state = AgentTurnState::Completed;
                        turn.error = None;
                    }
                    Err(ref err) => {
                        turn.state = AgentTurnState::Failed;
                        turn.error = Some(err.clone());
                    }
                }
            }
            turn.updated_at = current_millis();
            let snapshot = turn.clone();
            if inner.active_turn_id.as_deref() == Some(&turn_id) {
                inner.active_turn_id = None;
            }
            snapshot
        };
        emit_agent_turn_changed(&app, &completed);

        if let Err(err) = result {
            emit_runtime_error(&app, "agent", err.clone(), true);
            return Err(err);
        }

        Ok(completed)
    }

    fn mark_utterance_submitted(
        &self,
        app: &AppHandle,
        utterance_id: &str,
        turn_id: String,
    ) -> Result<(), String> {
        let event = {
            let mut inner = self.inner.lock();
            let utterance = inner
                .utterances
                .get_mut(utterance_id)
                .ok_or_else(|| format!("Unknown utterance id: {utterance_id}"))?;
            utterance.status = UtteranceStatus::SubmittedToAgent;
            utterance.turn_id = Some(turn_id.clone());
            VoiceUtteranceEvent {
                kind: VoiceUtteranceKind::SubmittedToAgent,
                session_id: utterance.session_id,
                utterance_id: utterance_id.to_string(),
                transcript: Some(utterance.transcript.clone()),
                original_transcript: Some(utterance.original_transcript.clone()),
                turn_id: Some(turn_id),
                error: None,
                created_at: current_millis(),
            }
        };
        emit_voice_utterance(app, event);
        Ok(())
    }

    fn mark_speech_from_tts(&self, app: &AppHandle, tts: &TtsStatusSnapshot) -> SpeechOutputStatus {
        let speech = {
            let mut inner = self.inner.lock();
            inner.speech.state = match tts.state {
                TtsState::Idle => SpeechOutputState::Idle,
                TtsState::Synthesizing => SpeechOutputState::Synthesizing,
                TtsState::Ready => SpeechOutputState::Ready,
                TtsState::Playing => SpeechOutputState::Playing,
                TtsState::Failed => SpeechOutputState::Failed,
            };
            if matches!(tts.state, TtsState::Idle | TtsState::Failed) {
                inner.speech.speech_id = None;
                inner.speech.source = None;
            }
            inner.speech.error = tts.error.clone();
            inner.speech.clone()
        };
        emit_speech_output_changed(app, &speech);
        speech
    }
}

pub fn emit_app_status_changed(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(status) = get_app_status(app.clone()).await {
            let _ = app.emit("app-status-changed", status);
        }
    });
}

pub fn emit_voice_session_changed(app: &AppHandle, status: &VoiceSessionStatus) {
    let _ = app.emit("voice-session-changed", status);
}

pub fn emit_agent_status_changed(app: &AppHandle, status: &AgentStatus) {
    let _ = app.emit("agent-status-changed", status);
}

pub fn emit_agent_turn_changed(app: &AppHandle, status: &AgentTurnStatus) {
    let _ = app.emit("agent-turn-changed", status);
}

pub fn emit_speech_output_changed(app: &AppHandle, status: &SpeechOutputStatus) {
    let _ = app.emit("speech-output-changed", status);
}

pub fn emit_voice_utterance(app: &AppHandle, event: VoiceUtteranceEvent) {
    let _ = app.emit("voice-utterance", event);
}

pub fn emit_runtime_error(
    app: &AppHandle,
    scope: impl Into<String>,
    message: impl Into<String>,
    recoverable: bool,
) {
    let _ = app.emit(
        "runtime-error",
        RuntimeErrorEvent {
            scope: scope.into(),
            message: message.into(),
            recoverable,
            created_at: current_millis(),
        },
    );
}

pub fn emit_speech_from_tts(app: &AppHandle, tts: &TtsStatusSnapshot) {
    let runtime = app.state::<BusinessRuntime>();
    runtime.mark_speech_from_tts(app, tts);
}

pub fn emit_speech_from_auto_tts(
    app: &AppHandle,
    tts: &TtsStatusSnapshot,
    status: AutoTtsLastStatus,
) {
    let runtime = app.state::<BusinessRuntime>();
    let speech = runtime.mark_speech_from_tts(app, tts);
    if matches!(status, AutoTtsLastStatus::Failed) {
        if let Some(error) = speech.error {
            emit_runtime_error(app, "speech", error, true);
        }
    }
}

#[tauri::command]
pub async fn get_app_status(app: AppHandle) -> Result<AppStatus, String> {
    let business = app.state::<BusinessRuntime>();
    let tts = app.state::<crate::tts::TtsRuntime>().status();
    let agent = app.state::<crate::acp::AcpRuntime>().status();
    let asr = match crate::asr::asr_status_runtime().await {
        Ok(status) => serde_json::to_value(status).map_err(|e| e.to_string())?,
        Err(error) => serde_json::json!({ "state": "failed", "error": error }),
    };
    let inner = business.inner.lock().clone();
    Ok(AppStatus {
        readiness: inner.app_readiness,
        error: inner.app_error,
        asr,
        tts,
        voice: inner.voice,
        agent,
        speech: inner.speech,
        preferences: inner.preferences,
    })
}

#[tauri::command]
pub async fn prepare_app(app: AppHandle) -> Result<AppStatus, String> {
    let business = app.state::<BusinessRuntime>();
    business.set_app_readiness(&app, AppReadiness::Initializing, None);

    let asr = crate::asr::prepare_asr_runtime(app.clone()).await;
    let tts = app
        .state::<crate::tts::TtsRuntime>()
        .prepare(Some(&app))
        .await;
    let errors = [asr.as_ref().err(), tts.as_ref().err()]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let readiness = if errors.is_empty() {
        AppReadiness::Ready
    } else if asr.is_ok() || tts.is_ok() {
        AppReadiness::Degraded
    } else {
        AppReadiness::Failed
    };
    business.set_app_readiness(&app, readiness, errors.first().cloned());
    get_app_status(app).await
}

#[tauri::command]
pub fn get_app_preferences(
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<AppPreferences, String> {
    Ok(runtime.preferences())
}

#[tauri::command]
pub fn set_app_preferences(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    preferences: AppPreferences,
) -> Result<AppPreferences, String> {
    let preferences = runtime.set_preferences(preferences);
    emit_app_status_changed(&app);
    emit_voice_session_changed(&app, &runtime.voice_status());
    emit_speech_output_changed(&app, &runtime.inner.lock().speech.clone());
    Ok(preferences)
}

#[tauri::command]
pub async fn start_voice_session(
    app: AppHandle,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<VoiceSessionStatus, String> {
    runtime.set_voice_status(&app, None, VoiceSessionState::Starting, None, None);
    if let Err(err) = crate::vad_commands::start_listening_runtime(
        app.clone(),
        vad_state.clone(),
        vad_config_state,
    )
    .await
    {
        let session_id = vad_state.current_active_session();
        runtime.set_voice_status(
            &app,
            session_id,
            VoiceSessionState::Failed,
            None,
            Some(err.clone()),
        );
        emit_runtime_error(&app, "voice", err.clone(), true);
        return Err(err);
    }
    let session_id = vad_state.current_active_session();
    Ok(runtime.set_voice_status(&app, session_id, VoiceSessionState::Listening, None, None))
}

#[tauri::command]
pub fn stop_voice_session(
    app: AppHandle,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<VoiceSessionStatus, String> {
    let session_id = vad_state.current_active_session();
    runtime.set_voice_status(&app, session_id, VoiceSessionState::Stopping, None, None);
    crate::vad_commands::stop_listening_runtime(app.clone(), vad_state)?;
    Ok(runtime.set_voice_status(&app, None, VoiceSessionState::Idle, None, None))
}

#[tauri::command]
pub fn pause_voice_session(
    app: AppHandle,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<VoiceSessionStatus, String> {
    let session_id = vad_state.current_active_session();
    let paused = crate::vad_commands::pause_listening_for_playback(vad_state)?;
    if !paused {
        return Err("No active voice session to pause".to_string());
    }
    Ok(runtime.set_voice_status(
        &app,
        session_id,
        VoiceSessionState::Paused,
        Some(VoicePauseReason::User),
        None,
    ))
}

#[tauri::command]
pub fn resume_voice_session(
    app: AppHandle,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<VoiceSessionStatus, String> {
    let session_id = vad_state.current_active_session();
    let resumed = crate::vad_commands::resume_listening_after_playback(vad_state)?;
    if !resumed {
        return Err("No paused voice session to resume".to_string());
    }
    Ok(runtime.set_voice_status(&app, session_id, VoiceSessionState::Listening, None, None))
}

#[tauri::command]
pub fn get_voice_session_status(
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<VoiceSessionStatus, String> {
    Ok(runtime.voice_status())
}

#[tauri::command]
pub fn update_voice_session_config(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    config: VoiceSessionConfig,
) -> Result<VoiceSessionStatus, String> {
    let status = {
        let mut inner = runtime.inner.lock();
        inner.preferences.voice = config.clone();
        inner.voice.config = config;
        inner.voice.clone()
    };
    emit_voice_session_changed(&app, &status);
    emit_app_status_changed(&app);
    Ok(status)
}

#[tauri::command]
pub async fn submit_transcript_to_agent(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    utterance_id: String,
) -> Result<AgentTurnStatus, String> {
    let transcript = {
        let inner = runtime.inner.lock();
        let utterance = inner
            .utterances
            .get(&utterance_id)
            .ok_or_else(|| format!("Unknown utterance id: {utterance_id}"))?;
        utterance.transcript.clone()
    };
    let turn = runtime
        .send_agent_message_internal(
            app.clone(),
            SendAgentMessageRequest {
                text: transcript,
                source: AgentMessageSource::Voice,
                utterance_id: Some(utterance_id.clone()),
            },
        )
        .await?;
    runtime.mark_utterance_submitted(&app, &utterance_id, turn.turn_id.clone())?;
    Ok(turn)
}

#[tauri::command]
pub async fn edit_and_submit_transcript(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    utterance_id: String,
    text: String,
) -> Result<AgentTurnStatus, String> {
    {
        let mut inner = runtime.inner.lock();
        let utterance = inner
            .utterances
            .get_mut(&utterance_id)
            .ok_or_else(|| format!("Unknown utterance id: {utterance_id}"))?;
        utterance.transcript = text.clone();
    }
    let turn = runtime
        .send_agent_message_internal(
            app.clone(),
            SendAgentMessageRequest {
                text,
                source: AgentMessageSource::EditedTranscript,
                utterance_id: Some(utterance_id.clone()),
            },
        )
        .await?;
    runtime.mark_utterance_submitted(&app, &utterance_id, turn.turn_id.clone())?;
    Ok(turn)
}

#[tauri::command]
pub fn discard_transcript(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    utterance_id: String,
) -> Result<VoiceUtteranceEvent, String> {
    let event = {
        let mut inner = runtime.inner.lock();
        let utterance = inner
            .utterances
            .get_mut(&utterance_id)
            .ok_or_else(|| format!("Unknown utterance id: {utterance_id}"))?;
        utterance.status = UtteranceStatus::Discarded;
        VoiceUtteranceEvent {
            kind: VoiceUtteranceKind::Discarded,
            session_id: utterance.session_id,
            utterance_id,
            transcript: Some(utterance.transcript.clone()),
            original_transcript: Some(utterance.original_transcript.clone()),
            turn_id: utterance.turn_id.clone(),
            error: None,
            created_at: current_millis(),
        }
    };
    emit_voice_utterance(&app, event.clone());
    Ok(event)
}

#[tauri::command]
pub async fn send_agent_message(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    request: SendAgentMessageRequest,
) -> Result<AgentTurnStatus, String> {
    runtime.send_agent_message_internal(app, request).await
}

#[tauri::command]
pub fn cancel_agent_turn(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    turn_id: String,
) -> Result<AgentTurnStatus, String> {
    let status = {
        let mut inner = runtime.inner.lock();
        let turn = inner
            .turns
            .get_mut(&turn_id)
            .ok_or_else(|| format!("Unknown or completed Agent turn: {turn_id}"))?;
        if !matches!(turn.state, AgentTurnState::Running) {
            return Err(format!("Agent turn is not running: {turn_id}"));
        }
        turn.state = AgentTurnState::Cancelled;
        turn.updated_at = current_millis();
        turn.clone()
    };
    emit_agent_turn_changed(&app, &status);
    Ok(status)
}

#[tauri::command]
pub async fn speak_text(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    tts_runtime: tauri::State<'_, crate::tts::TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
    request: SpeakTextRequest,
) -> Result<SpeechOutputStatus, String> {
    let text = request.text.trim().to_string();
    if text.is_empty() {
        return Err("speech text must not be empty".to_string());
    }
    {
        let mut inner = runtime.inner.lock();
        inner.speech.speech_id = Some(runtime.next_speech_id());
        inner.speech.source = Some(SpeechOutputSource::Text);
        inner.speech.state = SpeechOutputState::Synthesizing;
        inner.speech.error = None;
        emit_speech_output_changed(&app, &inner.speech);
    }
    tts_runtime
        .synthesize(Some(&app), text, tts_core::TtsConfig::default())
        .await?;
    tts_runtime
        .play_buffered(app.clone(), vad_state, vad_config_state)
        .await?;
    Ok(runtime.inner.lock().speech.clone())
}

#[tauri::command]
pub async fn speak_agent_result(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    tts_runtime: tauri::State<'_, crate::tts::TtsRuntime>,
    request: SpeakAgentResultRequest,
) -> Result<SpeechOutputStatus, String> {
    {
        let mut inner = runtime.inner.lock();
        inner.speech.speech_id = Some(runtime.next_speech_id());
        inner.speech.source = Some(SpeechOutputSource::AgentResult);
        inner.speech.state = SpeechOutputState::Synthesizing;
        inner.speech.error = None;
        emit_speech_output_changed(&app, &inner.speech);
    }
    tts_runtime
        .speak_agent_result(app.clone(), request.result_id, request.content)
        .await?;
    Ok(runtime.inner.lock().speech.clone())
}

#[tauri::command]
pub async fn stop_speech(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    tts_runtime: tauri::State<'_, crate::tts::TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
) -> Result<SpeechOutputStatus, String> {
    {
        let mut inner = runtime.inner.lock();
        inner.speech.state = SpeechOutputState::Stopping;
        emit_speech_output_changed(&app, &inner.speech);
    }
    tts_runtime.cancel_playback();
    if tts_runtime.take_paused_recording() {
        let _ = crate::vad_commands::resume_listening_after_playback_with_app(&app, vad_state);
    }
    tts_runtime.force_idle(Some(&app));
    Ok(runtime.inner.lock().speech.clone())
}

#[tauri::command]
pub fn get_speech_status(
    runtime: tauri::State<'_, BusinessRuntime>,
) -> Result<SpeechOutputStatus, String> {
    Ok(runtime.inner.lock().speech.clone())
}

#[tauri::command]
pub fn set_speech_preferences(
    app: AppHandle,
    runtime: tauri::State<'_, BusinessRuntime>,
    preferences: SpeechPreferences,
) -> Result<SpeechOutputStatus, String> {
    let speech = {
        let mut inner = runtime.inner.lock();
        inner.preferences.speech = preferences.clone();
        inner.speech.auto_speak_agent_results = preferences.auto_speak_agent_results;
        inner.speech.clone()
    };
    app.state::<crate::tts::TtsRuntime>()
        .set_auto_enabled(Some(&app), preferences.auto_speak_agent_results);
    emit_speech_output_changed(&app, &speech);
    emit_app_status_changed(&app);
    Ok(speech)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_serializes_with_camel_case_contract() {
        let status = VoiceSessionStatus {
            session_id: Some(42),
            state: VoiceSessionState::Paused,
            pause_reason: Some(VoicePauseReason::TtsPlayback),
            error: None,
            config: VoiceSessionConfig {
                input_mode: VoiceInputMode::ConfirmBeforeSend,
            },
        };

        let value = serde_json::to_value(status).unwrap();
        assert_eq!(value["sessionId"], 42);
        assert_eq!(value["state"], "paused");
        assert_eq!(value["pauseReason"], "ttsPlayback");
        assert_eq!(value["config"]["inputMode"], "confirmBeforeSend");
    }

    #[test]
    fn utterance_discard_updates_record_without_agent_turn() {
        let runtime = BusinessRuntime::default();
        let utterance_id = runtime.next_utterance_id();
        runtime.inner.lock().utterances.insert(
            utterance_id.clone(),
            VoiceUtteranceRecord {
                session_id: 7,
                transcript: "hello".to_string(),
                original_transcript: "hello".to_string(),
                status: UtteranceStatus::PendingConfirmation,
                turn_id: None,
            },
        );

        {
            let mut inner = runtime.inner.lock();
            let utterance = inner.utterances.get_mut(&utterance_id).unwrap();
            utterance.status = UtteranceStatus::Discarded;
        }

        assert_eq!(
            runtime.inner.lock().utterances[&utterance_id].status,
            UtteranceStatus::Discarded
        );
    }

    #[test]
    fn agent_turn_can_be_marked_cancelled() {
        let runtime = BusinessRuntime::default();
        let turn_id = runtime.next_turn_id();
        let now = current_millis();
        runtime.inner.lock().turns.insert(
            turn_id.clone(),
            AgentTurnStatus {
                turn_id: turn_id.clone(),
                state: AgentTurnState::Running,
                source: AgentMessageSource::Manual,
                utterance_id: None,
                created_at: now,
                updated_at: now,
                error: None,
            },
        );

        runtime.inner.lock().turns.get_mut(&turn_id).unwrap().state = AgentTurnState::Cancelled;
        assert_eq!(
            runtime.inner.lock().turns[&turn_id].state,
            AgentTurnState::Cancelled
        );
    }

    #[test]
    fn speech_status_maps_tts_states() {
        let runtime = BusinessRuntime::default();
        let mut snapshot = runtime.inner.lock().speech.clone();
        snapshot.state = SpeechOutputState::Playing;
        assert_eq!(snapshot.state, SpeechOutputState::Playing);
    }
}
