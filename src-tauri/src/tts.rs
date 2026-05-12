use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};
#[cfg(any(test, all(feature = "tts-mock", not(feature = "tts-moss-onnx"))))]
use tts_core::{AudioBuffer, PcmData, PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ};
use tts_core::{
    PlaybackBufferConfig, TtsConfig, TtsEngine, TtsError, TtsResult, TtsSynthesisEvent,
};

use crate::audio::{playback_buffer_from_tts, AudioOutput};
use crate::model_paths::{
    resolve_tts_model_path, resolve_tts_model_path_with_app, ModelPathSnapshot,
};
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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTtsStreamEvent {
    pub run_id: String,
    pub kind: DebugTtsStreamEventKind,
    pub buffer_state: DebugTtsBufferState,
    pub queued_duration_sec: f64,
    pub playback_position_sec: f64,
    pub pending_duration_sec: f64,
    pub output_queued_duration_sec: f64,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugTtsStreamEventKind {
    Started,
    Chunk,
    End,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugTtsBufferState {
    InitialBuffering,
    Playing,
    Rebuffering,
    Draining,
    Ended,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugTtsStreamResult {
    pub run_id: String,
    pub status: TtsStatusSnapshot,
}

impl DebugTtsStreamEvent {
    fn new(run_id: String, kind: DebugTtsStreamEventKind) -> Self {
        Self {
            run_id,
            kind,
            buffer_state: DebugTtsBufferState::InitialBuffering,
            queued_duration_sec: 0.0,
            playback_position_sec: 0.0,
            pending_duration_sec: 0.0,
            output_queued_duration_sec: 0.0,
            progress: 0.0,
            sequence: None,
            error: None,
        }
    }

    fn progress(
        run_id: String,
        kind: DebugTtsStreamEventKind,
        buffer_state: DebugTtsBufferState,
        queued_duration_sec: f64,
        playback_position_sec: f64,
        pending_duration_sec: f64,
        output_queued_duration_sec: f64,
    ) -> Self {
        let progress = if queued_duration_sec > 0.0 {
            (playback_position_sec / queued_duration_sec).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            run_id,
            kind,
            buffer_state,
            queued_duration_sec,
            playback_position_sec,
            pending_duration_sec,
            output_queued_duration_sec,
            progress,
            sequence: None,
            error: None,
        }
    }

    fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
}

fn emit_debug_tts_stream_event(app: &AppHandle, event: DebugTtsStreamEvent) {
    let _ = app.emit("debug-tts-stream", event);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugTtsPlaybackState {
    InitialBuffering,
    Playing,
    Rebuffering,
    Draining,
    Ended,
}

impl DebugTtsPlaybackState {
    fn as_event_state(self) -> DebugTtsBufferState {
        match self {
            Self::InitialBuffering => DebugTtsBufferState::InitialBuffering,
            Self::Playing => DebugTtsBufferState::Playing,
            Self::Rebuffering => DebugTtsBufferState::Rebuffering,
            Self::Draining => DebugTtsBufferState::Draining,
            Self::Ended => DebugTtsBufferState::Ended,
        }
    }
}

#[cfg(test)]
mod debug_playback_buffer_tests {
    use super::*;

    fn buffer(ms: u64) -> crate::audio::PlaybackBuffer {
        let samples =
            PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS as usize * ms as usize / 1_000;
        crate::audio::PlaybackBuffer::from_samples(vec![0.0; samples])
    }

    #[test]
    fn waits_for_adaptive_initial_buffer_before_playing() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        playback.push(buffer(300), 1);
        playback.update_for_output(0.0);
        assert_eq!(playback.state, DebugTtsPlaybackState::InitialBuffering);
        assert!(!playback.should_flush_pending());

        playback.push(buffer(300), 2);
        playback.update_for_output(0.0);
        assert_eq!(playback.state, DebugTtsPlaybackState::Playing);
        assert!(playback.should_flush_pending());
    }

    #[test]
    fn stable_chunk_arrivals_do_not_raise_target() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        let start = Instant::now();
        let initial_target = playback.dynamic_target_sec();
        for sequence in 0..8 {
            playback.push_at(
                buffer(100),
                sequence,
                start + Duration::from_millis(sequence * 100),
            );
        }

        assert!(playback.dynamic_target_sec() <= initial_target + 0.001);
    }

    #[test]
    fn jittery_chunk_arrivals_raise_target_without_exceeding_cap() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        let start = Instant::now();
        let initial_target = playback.dynamic_target_sec();
        let offsets = [0, 80, 420, 500, 940, 1_020, 1_480, 1_560];
        for (sequence, offset) in offsets.into_iter().enumerate() {
            playback.push_at(
                buffer(100),
                sequence as u64,
                start + Duration::from_millis(offset),
            );
        }

        assert!(playback.dynamic_target_sec() > initial_target);
        assert!(playback.dynamic_target_sec() <= 1.800);
    }

    #[test]
    fn low_water_rebuffering_waits_for_dynamic_target_then_resumes() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        playback.push(buffer(600), 1);
        playback.update_for_output(0.0);
        while playback.pop_pending().is_some() {}

        playback.update_for_output(0.100);
        assert_eq!(playback.state, DebugTtsPlaybackState::Rebuffering);
        let target = playback.rebuffer_resume_target_sec();
        playback.push(buffer(((target * 1_000.0) as u64).saturating_sub(100)), 2);
        playback.update_for_output(0.100);
        assert_eq!(playback.state, DebugTtsPlaybackState::Rebuffering);

        playback.push(buffer(160), 3);
        playback.update_for_output(0.100);
        assert_eq!(playback.state, DebugTtsPlaybackState::Playing);
    }

    #[test]
    fn final_chunk_drains_even_below_adaptive_target() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        playback.push(buffer(120), 1);
        playback.mark_finished();
        playback.update_for_output(0.0);

        assert_eq!(playback.state, DebugTtsPlaybackState::Playing);
        assert!(playback.should_flush_pending());
    }

    #[test]
    fn stable_playback_lowers_target_but_not_below_floor() {
        let mut playback = AdaptiveTtsJitterBuffer::new(PlaybackBufferConfig::default());
        playback.increase_target(0.600);
        playback.state = DebugTtsPlaybackState::Playing;
        for _ in 0..500 {
            playback.update_for_output(1.200);
        }

        assert!(playback.dynamic_target_sec() < 1.200);
        assert!(playback.dynamic_target_sec() >= playback.min_target_sec);
    }
}

#[derive(Debug, Clone)]
struct PendingPlaybackChunk {
    buffer: crate::audio::PlaybackBuffer,
    sequence: u64,
}

#[derive(Debug)]
struct AdaptiveTtsJitterBuffer {
    config: PlaybackBufferConfig,
    state: DebugTtsPlaybackState,
    pending: VecDeque<PendingPlaybackChunk>,
    pending_duration_sec: f64,
    total_audio_duration_sec: f64,
    latest_sequence: Option<u64>,
    synthesis_finished: bool,
    dynamic_target_sec: f64,
    min_target_sec: f64,
    max_target_sec: f64,
    arrival_interval_ewma_sec: Option<f64>,
    jitter_ewma_sec: f64,
    last_arrival_at: Option<Instant>,
    consecutive_low_water: u32,
    stable_ticks: u32,
    rebuffer_count: u32,
}

impl AdaptiveTtsJitterBuffer {
    fn new(config: PlaybackBufferConfig) -> Self {
        let initial_target_sec = config.initial_buffer_ms as f64 / 1_000.0;
        let target_floor_sec = config.rebuffer_threshold_ms as f64 / 1_000.0 + 0.150;
        let min_target_sec = initial_target_sec.min(0.400).max(target_floor_sec);
        let max_target_sec = (config.rebuffer_target_ms as f64 / 1_000.0)
            .max(initial_target_sec)
            .max(1.800);
        Self {
            config,
            state: DebugTtsPlaybackState::InitialBuffering,
            pending: VecDeque::new(),
            pending_duration_sec: 0.0,
            total_audio_duration_sec: 0.0,
            latest_sequence: None,
            synthesis_finished: false,
            dynamic_target_sec: initial_target_sec.clamp(min_target_sec, max_target_sec),
            min_target_sec,
            max_target_sec,
            arrival_interval_ewma_sec: None,
            jitter_ewma_sec: 0.0,
            last_arrival_at: None,
            consecutive_low_water: 0,
            stable_ticks: 0,
            rebuffer_count: 0,
        }
    }

    fn push(&mut self, buffer: crate::audio::PlaybackBuffer, sequence: u64) {
        self.record_arrival(Instant::now());
        let duration = buffer.duration().as_secs_f64();
        self.pending_duration_sec += duration;
        self.total_audio_duration_sec += duration;
        self.latest_sequence = Some(sequence);
        self.pending
            .push_back(PendingPlaybackChunk { buffer, sequence });
    }

    #[cfg(test)]
    fn push_at(
        &mut self,
        buffer: crate::audio::PlaybackBuffer,
        sequence: u64,
        arrival_at: Instant,
    ) {
        self.record_arrival(arrival_at);
        let duration = buffer.duration().as_secs_f64();
        self.pending_duration_sec += duration;
        self.total_audio_duration_sec += duration;
        self.latest_sequence = Some(sequence);
        self.pending
            .push_back(PendingPlaybackChunk { buffer, sequence });
    }

    fn record_arrival(&mut self, arrival_at: Instant) {
        if let Some(last_arrival_at) = self.last_arrival_at {
            let interval = arrival_at.duration_since(last_arrival_at).as_secs_f64();
            if let Some(ewma) = self.arrival_interval_ewma_sec {
                let jitter_sample = (interval - ewma).abs();
                self.arrival_interval_ewma_sec = Some(ewma * 0.85 + interval * 0.15);
                self.jitter_ewma_sec = self.jitter_ewma_sec * 0.80 + jitter_sample * 0.20;
            } else {
                self.arrival_interval_ewma_sec = Some(interval);
            }
            self.raise_target_for_jitter();
        }
        self.last_arrival_at = Some(arrival_at);
    }

    fn mark_finished(&mut self) {
        self.synthesis_finished = true;
    }

    fn update_for_output(&mut self, output_queued_sec: f64) {
        let low_water_sec = self.config.rebuffer_threshold_ms as f64 / 1_000.0;
        if matches!(self.state, DebugTtsPlaybackState::Playing) && output_queued_sec < low_water_sec
        {
            self.consecutive_low_water = self.consecutive_low_water.saturating_add(1);
            self.stable_ticks = 0;
            if self.consecutive_low_water >= 2 {
                self.increase_target(0.080);
            }
        } else if matches!(self.state, DebugTtsPlaybackState::Playing)
            && output_queued_sec >= self.dynamic_target_sec * 0.80
        {
            self.consecutive_low_water = 0;
            self.stable_ticks = self.stable_ticks.saturating_add(1);
            if self.stable_ticks >= 25 {
                self.decrease_target(0.020);
                self.stable_ticks = 0;
            }
        } else {
            self.consecutive_low_water = 0;
            self.stable_ticks = 0;
        }

        match self.state {
            DebugTtsPlaybackState::InitialBuffering => {
                if self.synthesis_finished || self.pending_duration_sec >= self.dynamic_target_sec {
                    self.state = DebugTtsPlaybackState::Playing;
                }
            }
            DebugTtsPlaybackState::Playing => {
                if self.synthesis_finished {
                    self.state = DebugTtsPlaybackState::Draining;
                } else if output_queued_sec < low_water_sec {
                    self.rebuffer_count = self.rebuffer_count.saturating_add(1);
                    self.increase_target(0.120);
                    self.state = DebugTtsPlaybackState::Rebuffering;
                }
            }
            DebugTtsPlaybackState::Rebuffering => {
                if self.synthesis_finished {
                    self.state = DebugTtsPlaybackState::Draining;
                } else if self.pending_duration_sec >= self.rebuffer_resume_target_sec() {
                    self.state = DebugTtsPlaybackState::Playing;
                }
            }
            DebugTtsPlaybackState::Draining | DebugTtsPlaybackState::Ended => {}
        }
    }

    fn should_flush_pending(&self) -> bool {
        matches!(
            self.state,
            DebugTtsPlaybackState::Playing | DebugTtsPlaybackState::Draining
        )
    }

    fn pop_pending(&mut self) -> Option<PendingPlaybackChunk> {
        let chunk = self.pending.pop_front()?;
        self.pending_duration_sec =
            (self.pending_duration_sec - chunk.buffer.duration().as_secs_f64()).max(0.0);
        Some(chunk)
    }

    fn finish_if_drained(&mut self, output_queued_sec: f64) {
        if self.synthesis_finished
            && self.pending.is_empty()
            && output_queued_sec <= 0.005
            && matches!(
                self.state,
                DebugTtsPlaybackState::Playing | DebugTtsPlaybackState::Draining
            )
        {
            self.state = DebugTtsPlaybackState::Ended;
        }
    }

    fn dynamic_target_sec(&self) -> f64 {
        self.dynamic_target_sec
    }

    fn rebuffer_resume_target_sec(&self) -> f64 {
        let recent_gap = self.arrival_interval_ewma_sec.unwrap_or(0.0);
        let jitter_margin = self.jitter_ewma_sec * 2.0 + 0.080;
        self.dynamic_target_sec
            .max(recent_gap + jitter_margin)
            .clamp(self.min_target_sec, self.max_target_sec)
    }

    fn raise_target_for_jitter(&mut self) {
        if self.jitter_ewma_sec > 0.180 {
            self.increase_target(0.180);
        } else if self.jitter_ewma_sec > 0.080 {
            self.increase_target(0.080);
        }
    }

    fn increase_target(&mut self, amount_sec: f64) {
        self.dynamic_target_sec = (self.dynamic_target_sec + amount_sec).min(self.max_target_sec);
    }

    fn decrease_target(&mut self, amount_sec: f64) {
        self.dynamic_target_sec = (self.dynamic_target_sec - amount_sec).max(self.min_target_sec);
    }

    fn debug_metrics(&self) -> (f64, f64, u32) {
        (
            self.dynamic_target_sec,
            self.jitter_ewma_sec,
            self.rebuffer_count,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoTtsLastStatus {
    Idle,
    Disabled,
    Speaking,
    SkippedDuplicate,
    SkippedMissingTag,
    SkippedInvalidTag,
    SkippedEmptyTag,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTtsStatusSnapshot {
    pub enabled: bool,
    pub is_playing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_result_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_result_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_spoken_result_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_skip_reason: Option<String>,
    pub last_status: AutoTtsLastStatus,
    pub tts: TtsStatusSnapshot,
}

#[derive(Debug, Clone)]
struct AutoTtsState {
    enabled: bool,
    is_playing: bool,
    latest_result_text: Option<String>,
    latest_result_key: Option<String>,
    latest_spoken_result_key: Option<String>,
    last_skip_reason: Option<String>,
    last_status: AutoTtsLastStatus,
}

struct TtsRuntimeInner {
    snapshot: TtsStatusSnapshot,
    latest_result: Option<TtsResult>,
    output: Option<AudioOutput>,
    playback_epoch: u64,
    paused_recording: bool,
    cancel_requested: bool,
    auto: AutoTtsState,
}

#[derive(Clone)]
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
        log::info!(
            "creating default TTS engine: model_dir={} source={:?}",
            model_path.engine_model_dir.display(),
            model_path.source
        );
        let config = MossModelConfig {
            model_dir: model_path.engine_model_dir,
        };
        match MossOnnxTtsEngine::new(config) {
            Ok(engine) => {
                log::info!("default TTS engine ready");
                Arc::new(engine)
            }
            Err(err) => {
                log::error!("failed to create default TTS engine: {err}");
                Arc::new(StartupErrorTtsEngine::new("moss-onnx-tts", err.to_string()))
            }
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
        log::info!(
            "creating TTS runtime: engine={} model_dir={} source={:?}",
            model.engine_name,
            model.model_dir,
            model.source
        );
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
                playback_epoch: 0,
                paused_recording: false,
                cancel_requested: false,
                auto: AutoTtsState {
                    enabled: true,
                    is_playing: false,
                    latest_result_text: None,
                    latest_result_key: None,
                    latest_spoken_result_key: None,
                    last_skip_reason: None,
                    last_status: AutoTtsLastStatus::Idle,
                },
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
            crate::business::emit_speech_from_tts(app, &snapshot);
        }
        log::debug!(
            "TTS state updated: state={:?} has_buffered_audio={} error={:?}",
            snapshot.state,
            snapshot.has_buffered_audio,
            snapshot.error
        );
    }

    fn auto_snapshot_locked(inner: &TtsRuntimeInner) -> AutoTtsStatusSnapshot {
        AutoTtsStatusSnapshot {
            enabled: inner.auto.enabled,
            is_playing: inner.auto.is_playing,
            latest_result_text: inner.auto.latest_result_text.clone(),
            latest_result_key: inner.auto.latest_result_key.clone(),
            latest_spoken_result_key: inner.auto.latest_spoken_result_key.clone(),
            last_skip_reason: inner.auto.last_skip_reason.clone(),
            last_status: inner.auto.last_status,
            tts: inner.snapshot.clone(),
        }
    }

    fn emit_auto_state(&self, app: Option<&AppHandle>) -> AutoTtsStatusSnapshot {
        let snapshot = {
            let inner = self.inner.lock();
            Self::auto_snapshot_locked(&inner)
        };
        if let Some(app) = app {
            let _ = app.emit("auto-tts-state", &snapshot);
            crate::business::emit_speech_from_auto_tts(app, &snapshot.tts, snapshot.last_status);
        }
        log::debug!(
            "auto TTS state updated: status={:?} enabled={} playing={} skip={:?}",
            snapshot.last_status,
            snapshot.enabled,
            snapshot.is_playing,
            snapshot.last_skip_reason
        );
        snapshot
    }

    pub fn status(&self) -> TtsStatusSnapshot {
        self.inner.lock().snapshot.clone()
    }

    fn stop_playback_locked(inner: &mut TtsRuntimeInner) {
        inner.playback_epoch = inner.playback_epoch.wrapping_add(1);
        inner.cancel_requested = true;
        if let Some(output) = inner.output.take() {
            output.clear();
        }
    }

    pub fn auto_status(&self) -> AutoTtsStatusSnapshot {
        let inner = self.inner.lock();
        Self::auto_snapshot_locked(&inner)
    }

    pub fn skip_auto_tts_missing_result(
        &self,
        app: Option<&AppHandle>,
        reason: impl Into<String>,
    ) -> AutoTtsStatusSnapshot {
        {
            let mut inner = self.inner.lock();
            inner.auto.is_playing = false;
            inner.auto.last_status = AutoTtsLastStatus::SkippedMissingTag;
            inner.auto.last_skip_reason = Some(reason.into());
        }
        self.emit_auto_state(app)
    }

    pub fn set_auto_enabled(
        &self,
        app: Option<&AppHandle>,
        enabled: bool,
    ) -> AutoTtsStatusSnapshot {
        {
            let mut inner = self.inner.lock();
            inner.auto.enabled = enabled;
            inner.auto.last_status = if enabled {
                inner.auto.last_skip_reason = None;
                AutoTtsLastStatus::Idle
            } else {
                inner.cancel_requested = true;
                inner.auto.last_skip_reason = Some("auto TTS is disabled".to_string());
                AutoTtsLastStatus::Disabled
            };
        }
        self.emit_auto_state(app)
    }

    pub async fn prepare(&self, app: Option<&AppHandle>) -> Result<TtsStatusSnapshot, String> {
        log::info!("TTS health check started: engine={}", self.engine_name);
        let healthy = self
            .engine
            .health_check()
            .await
            .map_err(|e| e.to_string())?;
        if healthy {
            log::info!("TTS health check passed");
            self.set_state(app, TtsState::Idle, None);
            Ok(self.status())
        } else {
            log::error!("TTS health check failed");
            self.set_state(
                app,
                TtsState::Failed,
                Some("TTS engine health check failed".to_string()),
            );
            Err("TTS engine health check failed".to_string())
        }
    }

    pub async fn synthesize(
        &self,
        app: Option<&AppHandle>,
        text: String,
        config: TtsConfig,
    ) -> Result<TtsStatusSnapshot, String> {
        let started = Instant::now();
        log::info!("TTS synthesis started: chars={}", text.chars().count());
        {
            let mut inner = self.inner.lock();
            Self::stop_playback_locked(&mut inner);
        }
        self.set_state(app, TtsState::Synthesizing, None);

        let result = self
            .engine
            .synthesize(&text, config)
            .await
            .map_err(|e| e.to_string())?;
        result.validate_for_playback().map_err(|e| e.to_string())?;
        let duration_ms = (result.audio.pcm.len_frames(result.audio.channels) as u128 * 1000)
            / result.audio.sample_rate_hz.max(1) as u128;
        log::info!(
            "TTS synthesis finished in {}ms: duration_ms={}",
            started.elapsed().as_millis(),
            duration_ms
        );

        {
            let mut inner = self.inner.lock();
            inner.latest_result = Some(result);
            inner.snapshot.has_buffered_audio = true;
        }

        self.set_state(app, TtsState::Ready, None);
        Ok(self.status())
    }

    pub fn cancel_playback(&self) {
        log::info!("TTS playback cancellation requested");
        let mut inner = self.inner.lock();
        Self::stop_playback_locked(&mut inner);
    }

    pub fn take_paused_recording(&self) -> bool {
        let mut inner = self.inner.lock();
        let paused = inner.paused_recording;
        inner.paused_recording = false;
        paused
    }

    pub fn force_idle(&self, app: Option<&AppHandle>) {
        self.set_state(app, TtsState::Idle, None);
    }

    pub async fn play_buffered(
        &self,
        app: AppHandle,
        vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
        vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
    ) -> Result<TtsStatusSnapshot, String> {
        log::info!("TTS playback requested");
        let buffer = {
            let mut inner = self.inner.lock();
            Self::stop_playback_locked(&mut inner);
            inner.cancel_requested = false;
            let result = inner
                .latest_result
                .as_ref()
                .ok_or_else(|| "No synthesized audio available".to_string())?;
            playback_buffer_from_tts(&result.audio).map_err(|e| e.to_string())?
        };

        let had_active_session = vad_state.has_active_session();
        if had_active_session {
            log::info!("pausing VAD recording for TTS playback");
            let paused = crate::vad_commands::pause_listening_for_playback_with_app(
                &app,
                vad_state.clone(),
            )?;
            self.inner.lock().paused_recording = paused;
        }

        self.set_state(Some(&app), TtsState::Playing, None);

        let output = match AudioOutput::new() {
            Ok(output) => output,
            Err(e) => {
                log::error!("failed to create audio output for TTS playback: {e}");
                self.set_state(Some(&app), TtsState::Failed, Some(e.to_string()));
                if self.inner.lock().paused_recording {
                    log::info!("resuming VAD recording after TTS output creation failure");
                    let _ = crate::vad_commands::resume_listening_after_playback_with_app(
                        &app,
                        vad_state.clone(),
                    );
                    self.inner.lock().paused_recording = false;
                }
                return Err("Failed to create output stream".to_string());
            }
        };

        let playback_deadline = Instant::now() + buffer.duration();
        log::info!(
            "TTS playback started: duration_ms={}",
            buffer.duration().as_millis()
        );
        output.enqueue(buffer);
        let playback_epoch = {
            let mut inner = self.inner.lock();
            inner.playback_epoch = inner.playback_epoch.wrapping_add(1);
            inner.cancel_requested = false;
            inner.output = Some(output);
            inner.playback_epoch
        };
        {
            let mut inner = self.inner.lock();
            inner.snapshot.has_buffered_audio = inner.latest_result.is_some();
        }

        loop {
            let should_stop = {
                let inner = self.inner.lock();
                inner.cancel_requested || inner.playback_epoch != playback_epoch
            };
            if should_stop {
                break;
            }
            if Instant::now() >= playback_deadline {
                break;
            }

            sleep(Duration::from_millis(20)).await;
        }

        {
            let mut inner = self.inner.lock();
            if inner.playback_epoch == playback_epoch {
                inner.output.take();
            }
        }

        let should_resume_recording = {
            let inner = self.inner.lock();
            inner.playback_epoch == playback_epoch && inner.paused_recording
        };
        if should_resume_recording {
            log::info!("resuming VAD recording after TTS playback");
            let _ = vad_config_state;
            crate::vad_commands::resume_listening_after_playback_with_app(&app, vad_state)?;
            let mut inner = self.inner.lock();
            if inner.playback_epoch == playback_epoch {
                inner.paused_recording = false;
            }
        }

        if self.inner.lock().playback_epoch == playback_epoch {
            self.set_state(Some(&app), TtsState::Idle, None);
        }
        log::info!("TTS playback finished");
        Ok(self.status())
    }

    pub async fn stream_play_debug(
        &self,
        app: AppHandle,
        run_id: String,
        text: String,
        config: TtsConfig,
    ) -> Result<DebugTtsStreamResult, String> {
        if text.trim().is_empty() {
            return Err("TTS text must not be empty".to_string());
        }
        let playback_config = PlaybackBufferConfig::from_stream_config(config.stream.as_ref());

        {
            let mut inner = self.inner.lock();
            Self::stop_playback_locked(&mut inner);
            inner.cancel_requested = false;
            inner.latest_result = None;
        }
        self.set_state(Some(&app), TtsState::Synthesizing, None);

        let output = match AudioOutput::new() {
            Ok(output) => output,
            Err(err) => {
                let message = err.to_string();
                self.set_state(Some(&app), TtsState::Failed, Some(message.clone()));
                emit_debug_tts_stream_event(
                    &app,
                    DebugTtsStreamEvent::new(run_id.clone(), DebugTtsStreamEventKind::Error)
                        .with_error(message.clone()),
                );
                return Err(message);
            }
        };
        let playback_epoch = {
            let mut inner = self.inner.lock();
            inner.playback_epoch = inner.playback_epoch.wrapping_add(1);
            inner.output = Some(output);
            inner.playback_epoch
        };
        self.set_state(Some(&app), TtsState::Playing, None);
        emit_debug_tts_stream_event(
            &app,
            DebugTtsStreamEvent::new(run_id.clone(), DebugTtsStreamEventKind::Started),
        );

        let (events_tx, mut events_rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = Arc::clone(&self.engine);
        let synth_text = text.clone();
        let mut synth_task = Some(tokio::spawn(async move {
            engine
                .synthesize_stream_events(
                    &synth_text,
                    config,
                    Box::new(move |event| {
                        let _ = events_tx.send(event);
                    }),
                )
                .await
        }));

        let mut playback = AdaptiveTtsJitterBuffer::new(playback_config);
        let mut first_audio_at: Option<Instant> = None;
        let mut last_event_at = Instant::now();
        let mut stream_result: Option<Result<TtsResult, TtsError>> = None;
        loop {
            if self.debug_stream_cancelled(playback_epoch) {
                break;
            }

            while let Ok(event) = events_rx.try_recv() {
                match event {
                    TtsSynthesisEvent::AudioChunk(chunk) => {
                        let buffer = playback_buffer_from_tts(&chunk.audio)
                            .map_err(|err| err.to_string())?;
                        playback.push(buffer, chunk.sequence);
                    }
                    TtsSynthesisEvent::End(_) => {
                        playback.mark_finished();
                    }
                    _ => {}
                }
            }

            if stream_result.is_none()
                && synth_task
                    .as_ref()
                    .map(|task| task.is_finished())
                    .unwrap_or(false)
            {
                let task = synth_task.take().expect("checked task exists");
                stream_result = Some(match task.await {
                    Ok(result) => result,
                    Err(err) => Err(TtsError::Other(err.to_string())),
                });
                playback.mark_finished();
            }

            let output_queued_sec = self
                .inner
                .lock()
                .output
                .as_ref()
                .map(|output| output.queued_duration().as_secs_f64())
                .unwrap_or(0.0);
            playback.update_for_output(output_queued_sec);
            if let Some(output) = self.inner.lock().output.as_ref() {
                output.configure_adaptive_playback(playback.dynamic_target_sec());
            }
            if playback.should_flush_pending() {
                while let Some(chunk) = playback.pop_pending() {
                    if first_audio_at.is_none() {
                        first_audio_at = Some(Instant::now());
                    }
                    if let Some(output) = self.inner.lock().output.as_ref() {
                        output.enqueue(chunk.buffer);
                    }
                    playback.latest_sequence = Some(chunk.sequence);
                }
            }

            let output_queued_sec = self
                .inner
                .lock()
                .output
                .as_ref()
                .map(|output| output.queued_duration().as_secs_f64())
                .unwrap_or(0.0);
            playback.finish_if_drained(output_queued_sec);
            let playback_position_sec = first_audio_at
                .map(|started| {
                    (started.elapsed().as_secs_f64()
                        - playback.pending_duration_sec
                        - output_queued_sec)
                        .max(0.0)
                })
                .unwrap_or(0.0);
            if last_event_at.elapsed() >= Duration::from_millis(100)
                || playback.latest_sequence.is_some()
                || matches!(playback.state, DebugTtsPlaybackState::Ended)
            {
                let (target_sec, jitter_sec, rebuffer_count) = playback.debug_metrics();
                log::debug!(
                    "adaptive TTS playback: state={:?} target_ms={} jitter_ms={} pending_ms={} output_ms={} rebuffer_count={}",
                    playback.state,
                    (target_sec * 1_000.0).round() as u64,
                    (jitter_sec * 1_000.0).round() as u64,
                    (playback.pending_duration_sec * 1_000.0).round() as u64,
                    (output_queued_sec * 1_000.0).round() as u64,
                    rebuffer_count
                );
                let kind = if matches!(playback.state, DebugTtsPlaybackState::Ended) {
                    DebugTtsStreamEventKind::End
                } else {
                    DebugTtsStreamEventKind::Chunk
                };
                let mut event = DebugTtsStreamEvent::progress(
                    run_id.clone(),
                    kind,
                    playback.state.as_event_state(),
                    playback.total_audio_duration_sec,
                    playback_position_sec,
                    playback.pending_duration_sec,
                    output_queued_sec,
                );
                if let Some(sequence) = playback.latest_sequence.take() {
                    event = event.with_sequence(sequence);
                }
                emit_debug_tts_stream_event(&app, event);
                last_event_at = Instant::now();
            }

            if matches!(playback.state, DebugTtsPlaybackState::Ended) {
                break;
            }

            sleep(Duration::from_millis(20)).await;
        }

        if stream_result.is_none() {
            if let Some(task) = synth_task.take() {
                stream_result = Some(match task.await {
                    Ok(result) => result,
                    Err(err) => Err(TtsError::Other(err.to_string())),
                });
            }
        }

        match stream_result {
            Some(Ok(result)) => {
                let mut inner = self.inner.lock();
                inner.latest_result = Some(result);
                inner.snapshot.has_buffered_audio = true;
            }
            Some(Err(err)) => {
                let message = err.to_string();
                self.set_state(Some(&app), TtsState::Failed, Some(message.clone()));
                emit_debug_tts_stream_event(
                    &app,
                    DebugTtsStreamEvent::new(run_id.clone(), DebugTtsStreamEventKind::Error)
                        .with_error(message.clone()),
                );
                return Err(message);
            }
            None if self.debug_stream_cancelled(playback_epoch) => {}
            None => {
                let message = "Streaming TTS ended without a synthesis result".to_string();
                self.set_state(Some(&app), TtsState::Failed, Some(message.clone()));
                emit_debug_tts_stream_event(
                    &app,
                    DebugTtsStreamEvent::new(run_id.clone(), DebugTtsStreamEventKind::Error)
                        .with_error(message.clone()),
                );
                return Err(message);
            }
        }

        {
            let mut inner = self.inner.lock();
            if inner.playback_epoch == playback_epoch {
                inner.output.take();
            }
        }
        emit_debug_tts_stream_event(
            &app,
            DebugTtsStreamEvent::progress(
                run_id.clone(),
                DebugTtsStreamEventKind::End,
                DebugTtsBufferState::Ended,
                playback.total_audio_duration_sec,
                playback.total_audio_duration_sec,
                0.0,
                0.0,
            ),
        );
        self.set_state(Some(&app), TtsState::Idle, None);
        Ok(DebugTtsStreamResult {
            run_id,
            status: self.status(),
        })
    }

    fn debug_stream_cancelled(&self, playback_epoch: u64) -> bool {
        let inner = self.inner.lock();
        inner.cancel_requested || inner.playback_epoch != playback_epoch
    }

    pub async fn speak_agent_result(
        &self,
        app: AppHandle,
        result_id: Option<String>,
        content: String,
    ) -> Result<AutoTtsStatusSnapshot, String> {
        self.speak_auto_result(app, result_id, content, false, false)
            .await
    }

    pub async fn speak_latest_auto_result(
        &self,
        app: AppHandle,
    ) -> Result<AutoTtsStatusSnapshot, String> {
        let (text, key) = {
            let inner = self.inner.lock();
            let text = inner
                .auto
                .latest_result_text
                .clone()
                .ok_or_else(|| "No latest Agent result available".to_string())?;
            (text, inner.auto.latest_result_key.clone())
        };
        self.speak_auto_result(app, key, text, true, true).await
    }

    async fn speak_auto_result(
        &self,
        app: AppHandle,
        result_key_or_id: Option<String>,
        content: String,
        force: bool,
        content_is_spoken_text: bool,
    ) -> Result<AutoTtsStatusSnapshot, String> {
        let raw_text = content.trim().to_string();
        log::debug!(
            "auto TTS requested: force={force} content_is_spoken_text={content_is_spoken_text} raw_len={}",
            raw_text.len()
        );
        if raw_text.is_empty() {
            let mut inner = self.inner.lock();
            inner.auto.last_status = AutoTtsLastStatus::SkippedMissingTag;
            inner.auto.last_skip_reason = Some("agent result is empty".to_string());
            drop(inner);
            return Ok(self.emit_auto_state(Some(&app)));
        }
        let speakable_text = match auto_tts_spoken_text(&raw_text, content_is_spoken_text) {
            Ok(text) => text,
            Err(reason) => {
                log::debug!("auto TTS skipped before synthesis: {reason}");
                let mut inner = self.inner.lock();
                inner.auto.last_status = reason.status();
                inner.auto.last_skip_reason = Some(reason.to_string());
                drop(inner);
                return Ok(self.emit_auto_state(Some(&app)));
            }
        };

        let key = if force {
            result_key_or_id.unwrap_or_else(|| auto_tts_result_key(None, &speakable_text))
        } else {
            auto_tts_result_key(result_key_or_id.as_deref(), &speakable_text)
        };
        {
            let mut inner = self.inner.lock();
            inner.auto.latest_result_text = Some(speakable_text.clone());
            inner.auto.latest_result_key = Some(key.clone());

            if !inner.auto.enabled && !force {
                log::debug!("auto TTS skipped because disabled: key={key}");
                inner.auto.last_status = AutoTtsLastStatus::Disabled;
                inner.auto.last_skip_reason = Some("auto TTS is disabled".to_string());
                drop(inner);
                return Ok(self.emit_auto_state(Some(&app)));
            }

            if !force && inner.auto.latest_spoken_result_key.as_deref() == Some(key.as_str()) {
                log::debug!("auto TTS skipped duplicate result: key={key}");
                inner.auto.last_status = AutoTtsLastStatus::SkippedDuplicate;
                inner.auto.last_skip_reason =
                    Some("extracted TTS text already matches latest spoken result".to_string());
                drop(inner);
                return Ok(self.emit_auto_state(Some(&app)));
            }

            inner.auto.is_playing = true;
            inner.auto.last_status = AutoTtsLastStatus::Speaking;
            inner.auto.last_skip_reason = None;
        }
        log::info!(
            "auto TTS synthesis/playback started: key={key} spoken_len={}",
            speakable_text.len()
        );
        self.emit_auto_state(Some(&app));

        let result = async {
            self.synthesize(Some(&app), speakable_text, TtsConfig::default())
                .await?;
            self.play_buffered(
                app.clone(),
                app.state::<crate::vad_commands::VadRecorderState>(),
                app.state::<crate::vad_commands::VadRuntimeConfigState>(),
            )
            .await
        }
        .await;

        {
            let mut inner = self.inner.lock();
            inner.auto.is_playing = false;
            if result.is_ok() && !inner.cancel_requested {
                inner.auto.latest_spoken_result_key = Some(key);
            }
            inner.auto.last_status = match &result {
                Ok(_) if inner.cancel_requested => AutoTtsLastStatus::Stopped,
                Ok(_) => AutoTtsLastStatus::Idle,
                Err(_) => AutoTtsLastStatus::Failed,
            };
        }
        match &result {
            Ok(_) => log::info!("auto TTS synthesis/playback finished"),
            Err(err) => log::error!("auto TTS synthesis/playback failed: {err}"),
        }
        let snapshot = self.emit_auto_state(Some(&app));
        result.map(|_| snapshot).inspect_err(|err| {
            let mut inner = self.inner.lock();
            inner.snapshot.error = Some(err.clone());
        })
    }
}

pub(crate) fn auto_tts_result_key(result_id: Option<&str>, content: &str) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    match result_id {
        Some(id) if !id.is_empty() => format!("{id}:{normalized}"),
        _ => normalized,
    }
}

fn auto_tts_spoken_text(
    content: &str,
    content_is_spoken_text: bool,
) -> Result<String, AgentTtsTagError> {
    if content_is_spoken_text {
        Ok(content.trim().to_string())
    } else {
        extract_agent_tts_text(content)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentTtsTagError {
    MissingTag,
    MultipleTags,
    IncompleteTag,
    EmptyTag,
    NestedTag,
    CaseMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentTtsReadiness {
    Complete,
    Pending,
    Invalid(AgentTtsTagError),
}

pub(crate) fn agent_tts_readiness(content: &str) -> AgentTtsReadiness {
    match extract_agent_tts_text(content) {
        Ok(_) => AgentTtsReadiness::Complete,
        Err(AgentTtsTagError::IncompleteTag) => AgentTtsReadiness::Pending,
        Err(reason) => AgentTtsReadiness::Invalid(reason),
    }
}

impl AgentTtsTagError {
    fn status(self) -> AutoTtsLastStatus {
        match self {
            AgentTtsTagError::MissingTag => AutoTtsLastStatus::SkippedMissingTag,
            AgentTtsTagError::EmptyTag => AutoTtsLastStatus::SkippedEmptyTag,
            AgentTtsTagError::MultipleTags
            | AgentTtsTagError::IncompleteTag
            | AgentTtsTagError::NestedTag
            | AgentTtsTagError::CaseMismatch => AutoTtsLastStatus::SkippedInvalidTag,
        }
    }
}

impl std::fmt::Display for AgentTtsTagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            AgentTtsTagError::MissingTag => "agent result is missing a <tts>...</tts> block",
            AgentTtsTagError::MultipleTags => "agent result contains multiple <tts> blocks",
            AgentTtsTagError::IncompleteTag => "agent result contains an incomplete <tts> block",
            AgentTtsTagError::EmptyTag => "agent result contains an empty <tts> block",
            AgentTtsTagError::NestedTag => "agent result contains nested <tts> tags",
            AgentTtsTagError::CaseMismatch => {
                "agent result contains case-mismatched TTS tags; use exact lowercase <tts>"
            }
        };
        f.write_str(reason)
    }
}

pub(crate) fn extract_agent_tts_text(content: &str) -> Result<String, AgentTtsTagError> {
    if has_case_mismatched_tts_tag(content) {
        return Err(AgentTtsTagError::CaseMismatch);
    }

    let mut matches = Vec::new();
    let mut search_from = 0;
    while let Some(relative_start) = content[search_from..].find("<tts>") {
        let start = search_from + relative_start;
        let inner_start = start + "<tts>".len();
        let Some(relative_end) = content[inner_start..].find("</tts>") else {
            return Err(AgentTtsTagError::IncompleteTag);
        };
        let end = inner_start + relative_end;
        let block_end = end + "</tts>".len();
        matches.push((inner_start, end, block_end));
        search_from = block_end;
    }

    if matches.is_empty() {
        if content.contains("</tts>") {
            return Err(AgentTtsTagError::IncompleteTag);
        }
        return Err(AgentTtsTagError::MissingTag);
    }

    if matches.len() > 1 {
        return Err(AgentTtsTagError::MultipleTags);
    }

    let (inner_start, inner_end, block_end) = matches[0];
    let inner = &content[inner_start..inner_end];
    if inner.contains("<tts>") || inner.contains("</tts>") {
        return Err(AgentTtsTagError::NestedTag);
    }

    if content[block_end..].contains("</tts>") {
        return Err(AgentTtsTagError::IncompleteTag);
    }

    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return Err(AgentTtsTagError::EmptyTag);
    }

    Ok(trimmed.to_string())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn strip_agent_tts_blocks(content: &str) -> String {
    let mut stripped = String::with_capacity(content.len());
    let mut search_from = 0;
    while let Some(relative_start) = content[search_from..].find("<tts>") {
        let start = search_from + relative_start;
        let inner_start = start + "<tts>".len();
        let Some(relative_end) = content[inner_start..].find("</tts>") else {
            break;
        };
        let end = inner_start + relative_end + "</tts>".len();
        stripped.push_str(&content[search_from..start]);
        search_from = end;
    }
    stripped.push_str(&content[search_from..]);
    stripped.trim().to_string()
}

fn has_case_mismatched_tts_tag(content: &str) -> bool {
    let lower = content.to_lowercase();
    let lowercase_count = content.matches("<tts>").count() + content.matches("</tts>").count();
    let any_case_count = lower.matches("<tts>").count() + lower.matches("</tts>").count();
    any_case_count > lowercase_count
}

#[cfg(feature = "tts-moss-onnx")]
fn default_tts_engine_with_model_dir(model_dir: std::path::PathBuf) -> Arc<dyn TtsEngine> {
    let config = MossModelConfig { model_dir };
    match MossOnnxTtsEngine::new(config) {
        Ok(engine) => {
            log::info!("TTS engine ready");
            Arc::new(engine)
        }
        Err(err) => {
            log::error!("failed to create TTS engine: {err}");
            Arc::new(StartupErrorTtsEngine::new("moss-onnx-tts", err.to_string()))
        }
    }
}

#[cfg(not(feature = "tts-moss-onnx"))]
fn default_tts_engine_with_model_dir(_model_dir: std::path::PathBuf) -> Arc<dyn TtsEngine> {
    default_tts_engine()
}

#[cfg(any(test, all(feature = "tts-mock", not(feature = "tts-moss-onnx"))))]
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

#[cfg(any(test, all(feature = "tts-mock", not(feature = "tts-moss-onnx"))))]
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
pub async fn debug_prepare_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
) -> Result<TtsStatusSnapshot, String> {
    runtime.prepare(Some(&app)).await
}

#[tauri::command]
pub fn debug_get_tts_status(
    runtime: tauri::State<'_, TtsRuntime>,
) -> Result<TtsStatusSnapshot, String> {
    Ok(runtime.status())
}

#[tauri::command]
pub async fn debug_synthesize_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    text: String,
    config: Option<TtsConfig>,
) -> Result<TtsStatusSnapshot, String> {
    runtime
        .synthesize(Some(&app), text, config.unwrap_or_default())
        .await
}

#[tauri::command]
pub async fn debug_play_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
) -> Result<TtsStatusSnapshot, String> {
    runtime
        .play_buffered(app, vad_state, vad_config_state)
        .await
}

#[tauri::command]
pub async fn debug_stream_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    run_id: String,
    text: String,
    config: Option<TtsConfig>,
) -> Result<DebugTtsStreamResult, String> {
    runtime
        .stream_play_debug(app, run_id, text, config.unwrap_or_default())
        .await
}

#[tauri::command]
pub async fn debug_cancel_tts_playback(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
) -> Result<TtsStatusSnapshot, String> {
    runtime.cancel_playback();

    let should_resume = runtime.inner.lock().paused_recording;
    if should_resume {
        let _ = vad_config_state;
        let _ = crate::vad_commands::resume_listening_after_playback_with_app(&app, vad_state);
        runtime.inner.lock().paused_recording = false;
    }

    runtime.set_state(Some(&app), TtsState::Idle, None);
    Ok(runtime.status())
}

#[tauri::command]
pub fn debug_get_auto_tts_status(
    runtime: tauri::State<'_, TtsRuntime>,
) -> Result<AutoTtsStatusSnapshot, String> {
    Ok(runtime.auto_status())
}

#[tauri::command]
pub fn debug_set_auto_tts_enabled(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    enabled: bool,
) -> Result<AutoTtsStatusSnapshot, String> {
    Ok(runtime.set_auto_enabled(Some(&app), enabled))
}

#[tauri::command]
pub async fn debug_stop_auto_tts(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
    vad_state: tauri::State<'_, crate::vad_commands::VadRecorderState>,
    vad_config_state: tauri::State<'_, crate::vad_commands::VadRuntimeConfigState>,
) -> Result<AutoTtsStatusSnapshot, String> {
    runtime.cancel_playback();

    let should_resume = runtime.inner.lock().paused_recording;
    if should_resume {
        let _ = vad_config_state;
        let _ = crate::vad_commands::resume_listening_after_playback_with_app(&app, vad_state);
        runtime.inner.lock().paused_recording = false;
    }

    {
        let mut inner = runtime.inner.lock();
        inner.auto.is_playing = false;
        inner.auto.last_status = AutoTtsLastStatus::Stopped;
    }
    runtime.set_state(Some(&app), TtsState::Idle, None);
    Ok(runtime.emit_auto_state(Some(&app)))
}

#[tauri::command]
pub async fn debug_speak_latest_result(
    app: AppHandle,
    runtime: tauri::State<'_, TtsRuntime>,
) -> Result<AutoTtsStatusSnapshot, String> {
    runtime.speak_latest_auto_result(app).await
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
            engine_model_dir: std::path::PathBuf::from(
                "models/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX",
            ),
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

    #[tokio::test]
    async fn synthesis_invalidates_prior_playback_epoch() {
        let runtime = TtsRuntime::new(Arc::new(MockTtsEngine));
        let starting_epoch = runtime.inner.lock().playback_epoch;

        runtime
            .synthesize(None, "hello".to_string(), TtsConfig::default())
            .await
            .expect("synthesis should work");

        assert!(runtime.inner.lock().playback_epoch > starting_epoch);
    }

    #[test]
    fn auto_tts_result_key_uses_id_and_normalized_content() {
        assert_eq!(
            auto_tts_result_key(Some("message-1"), "hello   world\nagain"),
            "message-1:hello world again"
        );
        assert_eq!(auto_tts_result_key(None, " hello "), "hello");
    }

    #[test]
    fn auto_tts_tracks_enabled_status_and_dedupe_key() {
        let runtime = TtsRuntime::new(Arc::new(MockTtsEngine));

        assert!(runtime.auto_status().enabled);
        let disabled = runtime.set_auto_enabled(None, false);
        assert!(!disabled.enabled);
        assert_eq!(disabled.last_status, AutoTtsLastStatus::Disabled);

        {
            let mut inner = runtime.inner.lock();
            inner.auto.latest_result_text = Some("Done".to_string());
            inner.auto.latest_result_key = Some("result-1:Done".to_string());
            inner.auto.latest_spoken_result_key = Some("result-1:Done".to_string());
            inner.auto.last_skip_reason = Some("duplicate".to_string());
            inner.auto.last_status = AutoTtsLastStatus::SkippedDuplicate;
        }

        let status = runtime.auto_status();
        assert_eq!(status.latest_result_text.as_deref(), Some("Done"));
        assert_eq!(status.latest_result_key.as_deref(), Some("result-1:Done"));
        assert_eq!(
            status.latest_spoken_result_key.as_deref(),
            Some("result-1:Done")
        );
        assert_eq!(status.last_skip_reason.as_deref(), Some("duplicate"));
        assert_eq!(status.last_status, AutoTtsLastStatus::SkippedDuplicate);
    }

    #[test]
    fn extracts_valid_single_tts_tag_and_strips_display_blocks() {
        let raw = "完成了。\n<tts>  我已经处理好了。  </tts>\n- 修改了测试";

        assert_eq!(
            extract_agent_tts_text(raw).as_deref(),
            Ok("我已经处理好了。")
        );
        assert_eq!(strip_agent_tts_blocks(raw), "完成了。\n\n- 修改了测试");
    }

    #[test]
    fn extracts_missing_tag_as_skip() {
        assert_eq!(
            extract_agent_tts_text("没有口播协议"),
            Err(AgentTtsTagError::MissingTag)
        );
    }

    #[test]
    fn extracts_multiple_tags_as_invalid() {
        let raw = "<tts>第一句</tts>\n正文\n<tts>第二句</tts>";

        assert_eq!(
            extract_agent_tts_text(raw),
            Err(AgentTtsTagError::MultipleTags)
        );
        assert_eq!(strip_agent_tts_blocks(raw), "正文");
    }

    #[test]
    fn extracts_incomplete_tags_as_invalid() {
        assert_eq!(
            extract_agent_tts_text("正文 <tts>没结束"),
            Err(AgentTtsTagError::IncompleteTag)
        );
        assert_eq!(
            extract_agent_tts_text("正文 </tts>"),
            Err(AgentTtsTagError::IncompleteTag)
        );
        assert_eq!(
            strip_agent_tts_blocks("正文 <tts>没结束"),
            "正文 <tts>没结束"
        );
    }

    #[test]
    fn extracts_empty_tag_as_empty_skip() {
        assert_eq!(
            extract_agent_tts_text("正文 <tts> \n\t </tts>"),
            Err(AgentTtsTagError::EmptyTag)
        );
    }

    #[test]
    fn extracts_nested_tags_as_invalid() {
        assert_eq!(
            extract_agent_tts_text("<tts>外层 <tts>内层</tts></tts>"),
            Err(AgentTtsTagError::NestedTag)
        );
    }

    #[test]
    fn extracts_case_mismatched_tags_as_invalid() {
        assert_eq!(
            extract_agent_tts_text("<TTS>不要播</TTS>"),
            Err(AgentTtsTagError::CaseMismatch)
        );
        assert_eq!(
            extract_agent_tts_text("<tts>不要播</TTS>"),
            Err(AgentTtsTagError::CaseMismatch)
        );
    }

    #[test]
    fn auto_tts_result_key_uses_extracted_spoken_text() {
        let raw = "展示文本 <tts>  只播这句  </tts>";
        let spoken = extract_agent_tts_text(raw).unwrap();

        assert_eq!(
            auto_tts_result_key(Some("result-1"), &spoken),
            "result-1:只播这句"
        );
    }

    #[test]
    fn latest_auto_result_replay_uses_stored_spoken_text_without_requiring_tags() {
        let speakable_text = auto_tts_spoken_text("只播这句", true).unwrap();
        let replay_key = auto_tts_result_key(Some("result-1"), &speakable_text);

        assert_eq!(speakable_text, "只播这句");
        assert_eq!(replay_key, "result-1:只播这句");
    }

    #[test]
    fn fresh_agent_result_still_requires_tts_tags() {
        assert_eq!(
            auto_tts_spoken_text("只播这句", false),
            Err(AgentTtsTagError::MissingTag)
        );
    }
}
