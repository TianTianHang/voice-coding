use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ndarray::{s, Array1, Array2, Array3, ArrayD, IxDyn};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::session::SessionInputValue;
use ort::value::{DynValue, TensorRef, Value};
use sentencepiece::SentencePieceProcessor;
use serde::Deserialize;
use tts_core::{
    AudioBuffer, PcmData, TtsConfig, TtsEngine, TtsError, TtsResult, PLAYBACK_CHANNELS,
    PLAYBACK_SAMPLE_RATE_HZ,
};

mod text;

use text::{MossTextPreprocessor, PreparedTextChunk};

mod reference_audio;

use reference_audio::{reference_audio_path, ReferenceAudio};

const DEFAULT_MODEL_DIR: &str = "../models/moss-tts/MOSS-TTS-Nano-100M-ONNX";
const MANIFEST_FILE: &str = "browser_poc_manifest.json";
const DEFAULT_VOICE: &str = "Junhao";

#[derive(Debug, Clone)]
pub struct MossModelConfig {
    pub model_dir: PathBuf,
}

impl MossModelConfig {
    pub fn from_env() -> Self {
        let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MODEL_DIR));
        Self { model_dir }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MossTtsError {
    #[error("missing required MOSS asset ({kind}): {path}")]
    MissingFile { kind: &'static str, path: PathBuf },

    #[error("failed to parse MOSS asset {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("invalid relative path for {field}: {raw}")]
    InvalidRelativePath { field: String, raw: String },

    #[error("MOSS metadata mismatch: {0}")]
    MetadataMismatch(String),

    #[error("MOSS tokenizer error: {0}")]
    Tokenizer(String),

    #[error("MOSS inference failed at {stage}: {detail}")]
    Inference { stage: &'static str, detail: String },

    #[error("unknown MOSS voice '{voice}'. Available voices include: {available}")]
    UnknownVoice { voice: String, available: String },

    #[error("unknown MOSS sampling mode '{mode}'. Available modes: fixed, greedy")]
    UnknownSamplingMode { mode: String },

    #[error("MOSS output format error: {0}")]
    OutputFormat(String),
}

impl From<MossTtsError> for TtsError {
    fn from(value: MossTtsError) -> Self {
        match value {
            MossTtsError::MissingFile { .. }
            | MossTtsError::Parse { .. }
            | MossTtsError::InvalidRelativePath { .. }
            | MossTtsError::MetadataMismatch(_) => TtsError::UnsupportedConfig(value.to_string()),
            MossTtsError::Tokenizer(_) => TtsError::SynthesisError(value.to_string()),
            MossTtsError::Inference { .. } => TtsError::SynthesisError(value.to_string()),
            MossTtsError::UnknownVoice { .. } | MossTtsError::UnknownSamplingMode { .. } => {
                TtsError::UnsupportedConfig(value.to_string())
            }
            MossTtsError::OutputFormat(_) => TtsError::UnsupportedConfig(value.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MossSamplingMode {
    Fixed,
    Greedy,
}

impl MossSamplingMode {
    fn from_config(config: &TtsConfig) -> Result<Self, MossTtsError> {
        let Some(mode) = config
            .moss
            .as_ref()
            .and_then(|moss| moss.sampling_mode.as_deref())
            .map(str::trim)
            .filter(|mode| !mode.is_empty())
        else {
            return Ok(Self::Fixed);
        };

        match mode.to_ascii_lowercase().as_str() {
            "fixed" => Ok(Self::Fixed),
            "greedy" => Ok(Self::Greedy),
            _ => Err(MossTtsError::UnknownSamplingMode {
                mode: mode.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MossAssets {
    #[allow(dead_code)]
    pub manifest_path: PathBuf,
    #[allow(dead_code)]
    pub tts_meta_path: PathBuf,
    #[allow(dead_code)]
    pub codec_meta_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub tts_files: HashMap<String, PathBuf>,
    pub codec_files: HashMap<String, PathBuf>,
    manifest: Manifest,
    tts_meta: TtsMeta,
    codec_meta: CodecMeta,
}

impl MossAssets {
    pub fn load(config: MossModelConfig) -> Result<Self, MossTtsError> {
        let manifest_path = config.model_dir.join(MANIFEST_FILE);
        ensure_file(&manifest_path, "manifest")?;
        let manifest: Manifest = read_json(&manifest_path)?;

        let tts_meta_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.tts_meta",
            &manifest.model_files.tts_meta,
        )?;
        let codec_meta_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.codec_meta",
            &manifest.model_files.codec_meta,
        )?;
        let tokenizer_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.tokenizer_model",
            &manifest.model_files.tokenizer_model,
        )?;
        ensure_file(&tts_meta_path, "tts meta")?;
        ensure_file(&codec_meta_path, "codec meta")?;
        ensure_file(&tokenizer_path, "tokenizer")?;

        let tts_meta: TtsMeta = read_json(&tts_meta_path)?;
        let codec_meta: CodecMeta = read_json(&codec_meta_path)?;
        validate_meta_consistency(&manifest, &tts_meta, &codec_meta)?;

        let tts_files = validate_model_files(
            tts_meta_path.parent().unwrap_or_else(|| Path::new("")),
            "tts",
            &tts_meta.files,
            &tts_meta.external_data_files,
        )?;
        let codec_files = validate_model_files(
            codec_meta_path.parent().unwrap_or_else(|| Path::new("")),
            "codec",
            &codec_meta.files,
            &codec_meta.external_data_files,
        )?;

        Ok(Self {
            manifest_path,
            tts_meta_path,
            codec_meta_path,
            tokenizer_path,
            tts_files,
            codec_files,
            manifest,
            tts_meta,
            codec_meta,
        })
    }

    fn resolve_voice(&self, requested: Option<&str>) -> Result<&BuiltinVoice, MossTtsError> {
        let voice = requested
            .map(str::trim)
            .filter(|voice| !voice.is_empty())
            .unwrap_or(DEFAULT_VOICE);
        self.manifest
            .builtin_voices
            .iter()
            .find(|candidate| candidate.voice.eq_ignore_ascii_case(voice))
            .ok_or_else(|| MossTtsError::UnknownVoice {
                voice: voice.to_string(),
                available: self.available_voices_summary(),
            })
    }

    fn available_voices_summary(&self) -> String {
        self.manifest
            .builtin_voices
            .iter()
            .take(8)
            .map(|voice| voice.voice.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn n_vq(&self) -> usize {
        self.manifest.tts_config.n_vq as usize
    }

    fn row_width(&self) -> usize {
        self.n_vq() + 1
    }

    fn audio_codebook_size(&self) -> usize {
        self.manifest
            .tts_config
            .audio_codebook_sizes
            .first()
            .copied()
            .unwrap_or(1024) as usize
    }

    fn max_new_frames(&self) -> usize {
        self.manifest.generation_defaults.max_new_frames as usize
    }

    fn text_row(&self, token_id: i64) -> Vec<i64> {
        let mut row = vec![self.manifest.tts_config.audio_pad_token_id; self.row_width()];
        row[0] = token_id;
        row
    }

    fn audio_row(&self, codes: &[i64], slot_token_id: i64) -> Vec<i64> {
        let mut row = vec![self.manifest.tts_config.audio_pad_token_id; self.row_width()];
        row[0] = slot_token_id;
        for (index, code) in codes.iter().take(self.n_vq()).enumerate() {
            row[index + 1] = *code;
        }
        row
    }

    fn build_voice_clone_request_rows(
        &self,
        text_token_ids: impl IntoIterator<Item = i64>,
        prompt_audio_codes: &[Vec<i64>],
    ) -> Result<MossRequestRows, MossTtsError> {
        let mut rows = Vec::new();
        for token_id in self
            .manifest
            .prompt_templates
            .user_prompt_prefix_token_ids
            .iter()
            .copied()
            .chain(std::iter::once(self.manifest.tts_config.audio_start_token_id))
        {
            rows.push(self.text_row(token_id));
        }
        for codes in prompt_audio_codes {
            rows.push(self.audio_row(codes, self.manifest.tts_config.audio_user_slot_token_id));
        }
        for token_id in std::iter::once(self.manifest.tts_config.audio_end_token_id)
            .chain(
                self.manifest
                    .prompt_templates
                    .user_prompt_after_reference_token_ids
                    .iter()
                    .copied(),
            )
            .chain(text_token_ids)
            .chain(
                self.manifest
                    .prompt_templates
                    .assistant_prompt_prefix_token_ids
                    .iter()
                    .copied(),
            )
            .chain(std::iter::once(self.manifest.tts_config.audio_start_token_id))
        {
            rows.push(self.text_row(token_id));
        }
        if rows.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "tts_prompt",
                detail: "MOSS prompt rows are empty".to_string(),
            });
        }
        Ok(MossRequestRows {
            attention_mask: vec![1; rows.len()],
            rows,
        })
    }
}

struct MossRequestRows {
    rows: Vec<Vec<i64>>,
    attention_mask: Vec<i64>,
}

pub struct MossOnnxTtsEngine {
    assets: MossAssets,
    tokenizer: MossTokenizer,
    text_preprocessor: MossTextPreprocessor,
    sessions: Arc<Mutex<Option<MossSessions>>>,
}

struct PreparedSynthesis {
    assets: MossAssets,
    prompt_audio_codes: Vec<Vec<i64>>,
    chunks: Vec<PreparedTextChunk>,
    sampling_mode: MossSamplingMode,
    reference_audio: Option<ReferenceAudio>,
}

enum MossTokenizer {
    SentencePiece(SentencePieceProcessor),
    HuggingFace(Box<tokenizers::Tokenizer>),
}

impl MossTokenizer {
    fn load(path: &Path) -> Result<Self, MossTtsError> {
        if path.extension().and_then(|extension| extension.to_str()) == Some("model") {
            return SentencePieceProcessor::open(path)
                .map(Self::SentencePiece)
                .map_err(|e| MossTtsError::Tokenizer(e.to_string()));
        }

        tokenizers::Tokenizer::from_file(path)
            .map(|tokenizer| Self::HuggingFace(Box::new(tokenizer)))
            .map_err(|e| MossTtsError::Tokenizer(e.to_string()))
    }

    fn encode_ids(&self, text: &str) -> Result<Vec<i64>, MossTtsError> {
        match self {
            Self::SentencePiece(tokenizer) => tokenizer
                .encode(text)
                .map(|pieces| pieces.into_iter().map(|piece| piece.id as i64).collect())
                .map_err(|e| MossTtsError::Tokenizer(e.to_string())),
            Self::HuggingFace(tokenizer) => tokenizer
                .encode(text, false)
                .map(|encoding| encoding.get_ids().iter().map(|id| *id as i64).collect())
                .map_err(|e| MossTtsError::Tokenizer(e.to_string())),
        }
    }
}

impl MossOnnxTtsEngine {
    pub fn from_env() -> Result<Self, MossTtsError> {
        Self::new(MossModelConfig::from_env())
    }

    pub fn new(config: MossModelConfig) -> Result<Self, MossTtsError> {
        let assets = MossAssets::load(config)?;
        let tokenizer = MossTokenizer::load(&assets.tokenizer_path)?;
        Ok(Self {
            assets,
            tokenizer,
            text_preprocessor: MossTextPreprocessor::default(),
            sessions: Arc::new(Mutex::new(None)),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn validate_output_contract(&self, result: &TtsResult) -> Result<(), MossTtsError> {
        if result.audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ {
            return Err(MossTtsError::OutputFormat(format!(
                "expected {}Hz, got {}Hz",
                PLAYBACK_SAMPLE_RATE_HZ, result.audio.sample_rate_hz
            )));
        }
        if result.audio.channels != PLAYBACK_CHANNELS {
            return Err(MossTtsError::OutputFormat(format!(
                "expected {} channels, got {}",
                PLAYBACK_CHANNELS, result.audio.channels
            )));
        }
        result
            .audio
            .validate()
            .map_err(|e| MossTtsError::OutputFormat(e.to_string()))
    }

    #[cfg(test)]
    fn from_assets_for_test(assets: MossAssets) -> Self {
        let tokenizer = MossTokenizer::HuggingFace(Box::new(tokenizers::Tokenizer::new(
            tokenizers::models::bpe::BPE::default(),
        )));
        Self {
            assets,
            tokenizer,
            text_preprocessor: MossTextPreprocessor::default(),
            sessions: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl TtsEngine for MossOnnxTtsEngine {
    fn engine_name(&self) -> &str {
        "moss-onnx-tts"
    }

    async fn synthesize(&self, text: &str, config: TtsConfig) -> tts_core::Result<TtsResult> {
        if text.trim().is_empty() {
            return Err(TtsError::InvalidInput("text must not be empty".to_string()));
        }
        let sampling_mode = MossSamplingMode::from_config(&config)?;
        let voice = self.assets.resolve_voice(config.voice.as_deref())?;
        let reference_audio = reference_audio_path(&config)
            .map(|path| ReferenceAudio::from_wav_path(&path))
            .transpose()?;
        let chunks = self
            .text_preprocessor
            .prepare(text, |chunk| self.tokenizer.encode_ids(chunk))?;
        if chunks.is_empty() {
            return Err(TtsError::InvalidInput(
                "text produced no speakable MOSS chunks".to_string(),
            ));
        }

        let prepared = PreparedSynthesis {
            assets: self.assets.clone(),
            prompt_audio_codes: voice.prompt_audio_codes.clone(),
            chunks,
            sampling_mode,
            reference_audio,
        };
        let sessions = Arc::clone(&self.sessions);
        let result = tokio::task::spawn_blocking(move || {
            synthesize_prepared_with_sessions(&sessions, prepared)
        })
        .await
        .map_err(|e| MossTtsError::Inference {
            stage: "inference_worker",
            detail: e.to_string(),
        })??;
        self.validate_output_contract(&result)?;
        Ok(result)
    }

    async fn health_check(&self) -> tts_core::Result<bool> {
        let assets = self.assets.clone();
        let sessions = Arc::clone(&self.sessions);
        tokio::task::spawn_blocking(move || {
            let mut sessions = sessions.lock().map_err(|e| MossTtsError::Inference {
                stage: "session_lock",
                detail: e.to_string(),
            })?;
            if sessions.is_none() {
                *sessions = Some(MossSessions::load(&assets)?);
            }
            Ok::<(), MossTtsError>(())
        })
        .await
        .map_err(|e| MossTtsError::Inference {
            stage: "inference_worker",
            detail: e.to_string(),
        })??;
        Ok(true)
    }
}

fn synthesize_prepared_with_sessions(
    sessions: &Mutex<Option<MossSessions>>,
    prepared: PreparedSynthesis,
) -> Result<TtsResult, MossTtsError> {
    let mut sessions = sessions.lock().map_err(|e| MossTtsError::Inference {
        stage: "session_lock",
        detail: e.to_string(),
    })?;
    if sessions.is_none() {
        *sessions = Some(MossSessions::load(&prepared.assets)?);
    }
    let sessions = sessions.as_mut().ok_or_else(|| MossTtsError::Inference {
        stage: "session_init",
        detail: "MOSS sessions were not initialized".to_string(),
    })?;
    let prompt_audio_codes = if let Some(reference_audio) = prepared.reference_audio {
        sessions.encode_reference_audio(reference_audio)?
    } else {
        prepared.prompt_audio_codes
    };
    synthesize_chunks(
        sessions,
        &prepared.assets,
        &prompt_audio_codes,
        prepared.chunks,
        prepared.sampling_mode,
    )
}

fn synthesize_chunks(
    sessions: &mut MossSessions,
    assets: &MossAssets,
    prompt_audio_codes: &[Vec<i64>],
    chunks: Vec<PreparedTextChunk>,
    sampling_mode: MossSamplingMode,
) -> Result<TtsResult, MossTtsError> {
    let mut results = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let request = assets.build_voice_clone_request_rows(chunk.token_ids, prompt_audio_codes)?;
        results.push(sessions.synthesize(assets, request, sampling_mode)?);
    }
    concatenate_tts_results(results)
}

fn concatenate_tts_results(results: Vec<TtsResult>) -> Result<TtsResult, MossTtsError> {
    let mut pcm = Vec::new();
    for result in results {
        if result.audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ
            || result.audio.channels != PLAYBACK_CHANNELS
        {
            return Err(MossTtsError::OutputFormat(format!(
                "chunk audio must be {}Hz stereo, got {}Hz/{}ch",
                PLAYBACK_SAMPLE_RATE_HZ, result.audio.sample_rate_hz, result.audio.channels
            )));
        }
        match result.audio.pcm {
            PcmData::F32(mut samples) => pcm.append(&mut samples),
            PcmData::I16(_) => {
                return Err(MossTtsError::OutputFormat(
                    "MOSS chunk audio must use f32 PCM".to_string(),
                ));
            }
        }
    }
    if pcm.is_empty() {
        return Err(MossTtsError::Inference {
            stage: "tts_concat_chunks",
            detail: "MOSS chunks produced no audio".to_string(),
        });
    }
    Ok(TtsResult {
        audio: AudioBuffer {
            sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
            channels: PLAYBACK_CHANNELS,
            pcm: PcmData::F32(pcm),
        },
    })
}

#[derive(Debug, Default)]
struct PcmChunkBuffer {
    chunks: Vec<Vec<f32>>,
}

impl PcmChunkBuffer {
    fn push_chunk(&mut self, samples: Vec<f32>) {
        if !samples.is_empty() {
            self.chunks.push(samples);
        }
    }

    fn into_tts_result(self) -> Result<TtsResult, MossTtsError> {
        let mut pcm = Vec::new();
        for mut chunk in self.chunks {
            pcm.append(&mut chunk);
        }
        if pcm.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "codec_decode_step",
                detail: "streaming decode produced no PCM chunks".to_string(),
            });
        }
        if pcm.len() % PLAYBACK_CHANNELS as usize != 0 {
            return Err(MossTtsError::OutputFormat(
                "streaming decode PCM length is not aligned to stereo channels".to_string(),
            ));
        }
        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(pcm),
            },
        })
    }
}

fn codec_decode_step_unavailable(detail: String) -> MossTtsError {
    MossTtsError::Inference {
        stage: "codec_decode_step",
        detail,
    }
}

struct MossSessions {
    #[allow(dead_code)]
    prefill: Session,
    #[allow(dead_code)]
    decode_step: Session,
    #[allow(dead_code)]
    local_decoder: Session,
    #[allow(dead_code)]
    local_cached_step: Session,
    #[allow(dead_code)]
    local_fixed_sampled_frame: Session,
    codec_encode: Session,
    codec_decode_full: Session,
    #[allow(dead_code)]
    codec_decode_step: Session,
    codec_decode_step_state: CodecDecodeStepState,
}

impl MossSessions {
    fn load(assets: &MossAssets) -> Result<Self, MossTtsError> {
        let prefill = create_session(required_file(&assets.tts_files, "prefill")?, "tts.prefill")?;
        let decode_step = create_session(required_file(&assets.tts_files, "decode_step")?, "tts.decode_step")?;
        let local_decoder = create_session(required_file(&assets.tts_files, "local_decoder")?, "tts.local_decoder")?;
        let local_cached_step = create_session(required_file(&assets.tts_files, "local_cached_step")?, "tts.local_cached_step")?;
        let local_fixed_sampled_frame = create_session(
            required_file(&assets.tts_files, "local_fixed_sampled_frame")?,
            "tts.local_fixed_sampled_frame",
        )?;
        let codec_decode_full = create_session(
            required_file(&assets.codec_files, "decode_full")?,
            "codec.decode_full",
        )?;
        let codec_encode = create_session(
            required_file(&assets.codec_files, "encode")?,
            "codec.encode",
        )?;
        let codec_decode_step = create_session(
            required_file(&assets.codec_files, "decode_step")?,
            "codec.decode_step",
        )?;

        validate_session_io(
            &prefill,
            "tts.prefill",
            ["input_ids", "attention_mask"],
            assets.tts_meta.onnx.prefill_output_names.iter(),
        )?;
        validate_session_io(
            &decode_step,
            "tts.decode_step",
            assets.tts_meta.onnx.decode_input_names.iter(),
            assets.tts_meta.onnx.decode_output_names.iter(),
        )?;
        validate_session_io(
            &local_decoder,
            "tts.local_decoder",
            &["global_hidden", "text_token_id", "audio_prefix_token_ids"],
            &["text_logits", "audio_logits"],
        )?;
        validate_session_io(
            &local_cached_step,
            "tts.local_cached_step",
            assets.tts_meta.onnx.local_cached_input_names.iter(),
            assets.tts_meta.onnx.local_cached_output_names.iter(),
        )?;
        validate_session_io(
            &local_fixed_sampled_frame,
            "tts.local_fixed_sampled_frame",
            &[
                "global_hidden",
                "repetition_seen_mask",
                "assistant_random_u",
                "audio_random_u",
            ],
            &["should_continue", "frame_token_ids"],
        )?;
        validate_session_io(
            &codec_encode,
            "codec.encode",
            assets.codec_meta.onnx.encode_input_names.iter(),
            assets.codec_meta.onnx.encode_output_names.iter(),
        )?;
        validate_session_io(
            &codec_decode_full,
            "codec.decode_full",
            assets.codec_meta.onnx.decode_input_names.iter(),
            assets.codec_meta.onnx.decode_output_names.iter(),
        )?;
        validate_session_io(
            &codec_decode_step,
            "codec.decode_step",
            assets.codec_meta.onnx.decode_step_input_names.iter(),
            assets.codec_meta.onnx.decode_step_output_names.iter(),
        )?;
        let codec_decode_step_state = CodecDecodeStepState::from_meta(&assets.codec_meta)?;

        Ok(Self {
            prefill,
            decode_step,
            local_decoder,
            local_cached_step,
            local_fixed_sampled_frame,
            codec_encode,
            codec_decode_full,
            codec_decode_step,
            codec_decode_step_state,
        })
    }

    fn encode_reference_audio(
        &mut self,
        audio: ReferenceAudio,
    ) -> Result<Vec<Vec<i64>>, MossTtsError> {
        if audio.sample_rate_hz != PLAYBACK_SAMPLE_RATE_HZ || audio.channels != PLAYBACK_CHANNELS {
            return Err(MossTtsError::Inference {
                stage: "reference_audio",
                detail: "reference audio was not normalized to 48kHz stereo".to_string(),
            });
        }
        let frames = audio.samples.len() / PLAYBACK_CHANNELS as usize;
        let waveform =
            Array3::from_shape_fn((1, PLAYBACK_CHANNELS as usize, frames), |(_, channel, frame)| {
                audio.samples[frame * PLAYBACK_CHANNELS as usize + channel]
            });
        let input_lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);
        let outputs = self
            .codec_encode
            .run(ort::inputs![
                "waveform" => TensorRef::from_array_view(waveform.view()).map_err(|e| MossTtsError::Inference { stage: "codec_encode", detail: e.to_string() })?,
                "input_lengths" => TensorRef::from_array_view(input_lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_encode", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_encode",
                detail: e.to_string(),
            })?;
        extract_audio_codes(&outputs, "codec_encode")
    }

    fn synthesize(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
    ) -> Result<TtsResult, MossTtsError> {
        let generated_frames = self.generate_audio_frames(assets, request, sampling_mode)?;
        self.decode_generated_frames(generated_frames)
    }

    fn decode_generated_frames(
        &mut self,
        generated_frames: Vec<Vec<i64>>,
    ) -> Result<TtsResult, MossTtsError> {
        match self.decode_step_buffered(&generated_frames) {
            Ok(result) => Ok(result),
            Err(_) => self.decode_full(generated_frames),
        }
    }

    fn decode_step_buffered(
        &mut self,
        audio_frames: &[Vec<i64>],
    ) -> Result<TtsResult, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(codec_decode_step_unavailable(
                "no audio frames to decode".to_string(),
            ));
        }
        let mut state = self.codec_decode_step_state.clone();
        let batch_size = state.batch_size;
        let mut buffer = PcmChunkBuffer::default();
        for batch in audio_frames.chunks(batch_size) {
            let chunk = self.run_codec_decode_step_batch(batch, &mut state)?;
            buffer.push_chunk(chunk);
        }
        buffer.into_tts_result()
    }

    fn run_codec_decode_step_batch(
        &mut self,
        audio_frames: &[Vec<i64>],
        state: &mut CodecDecodeStepState,
    ) -> Result<Vec<f32>, MossTtsError> {
        if audio_frames.is_empty() {
            return Err(codec_decode_step_unavailable(
                "decode step batch contains no frames".to_string(),
            ));
        }
        let frames = audio_frames.len();
        let quantizers = audio_frames.first().map(Vec::len).unwrap_or(0);
        let audio_codes = audio_frames
            .iter()
            .flatten()
            .map(|value| to_i32(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let codes =
            Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
                MossTtsError::Inference {
                    stage: "codec_decode_step",
                    detail: e.to_string(),
                }
            })?;
        let lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);

        let mut i32_state_tensors = Vec::new();
        let mut f32_state_tensors = Vec::new();
        state.collect_input_arrays(&mut i32_state_tensors, &mut f32_state_tensors)?;

        let mut inputs = ort::inputs![
            "audio_codes" => TensorRef::from_array_view(codes.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_step", detail: e.to_string() })?,
            "audio_code_lengths" => TensorRef::from_array_view(lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_step", detail: e.to_string() })?
        ];
        for (name, tensor) in &i32_state_tensors {
            inputs.push((
                name.clone().into(),
                SessionInputValue::from(
                    TensorRef::from_array_view(tensor.view()).map_err(|e| {
                        MossTtsError::Inference {
                            stage: "codec_decode_step",
                            detail: e.to_string(),
                        }
                    })?,
                ),
            ));
        }
        for (name, tensor) in &f32_state_tensors {
            inputs.push((
                name.clone().into(),
                SessionInputValue::from(
                    TensorRef::from_array_view(tensor.view()).map_err(|e| {
                        MossTtsError::Inference {
                            stage: "codec_decode_step",
                            detail: e.to_string(),
                        }
                    })?,
                ),
            ));
        }

        let outputs = self
            .codec_decode_step
            .run(inputs)
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_step",
                detail: e.to_string(),
            })?;
        let audio = extract_codec_audio_chunk(&outputs, "codec_decode_step")?;
        let owned_state_outputs = collect_codec_decode_state_outputs(&outputs, state)?;
        state.update_from_owned_outputs(&owned_state_outputs)?;
        Ok(audio)
    }

    fn generate_audio_frames(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
        sampling_mode: MossSamplingMode,
    ) -> Result<Vec<Vec<i64>>, MossTtsError> {
        let row_width = assets.row_width();
        let input_ids = Array3::from_shape_vec(
            (1, request.rows.len(), row_width),
            request
                .rows
                .concat()
                .into_iter()
                .map(to_i32)
                .collect::<Result<Vec<_>, _>>()?,
        )
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;
        let attention_mask = Array2::from_shape_vec(
            (1, request.attention_mask.len()),
            request
                .attention_mask
                .into_iter()
                .map(to_i32)
                .collect::<Result<Vec<_>, _>>()?,
        )
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;

        let mut outputs = self
            .prefill
            .run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(input_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?,
                "attention_mask" => TensorRef::from_array_view(attention_mask.view()).map_err(|e| MossTtsError::Inference { stage: "tts_prefill", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;

        let mut global_hidden = take_last_hidden_output(&mut outputs, "global_hidden", "tts_prefill")?;
        let mut decode_past = take_decode_present_outputs(&mut outputs, assets, "tts_prefill")?;
        let initial_past_valid_length = input_ids.shape()[1] as i64;
        drop(outputs);

        let mut frames = Vec::new();
        let mut previous_token_sets = vec![vec![false; assets.audio_codebook_size()]; assets.n_vq()];
        let mut rng = SimpleRng::new();

        for past_valid_length in (initial_past_valid_length..).take(assets.max_new_frames()) {
            let fixed = match sampling_mode {
                MossSamplingMode::Fixed => {
                    self.run_local_fixed_sampled_frame(
                        &global_hidden,
                        &previous_token_sets,
                        &mut rng,
                        assets,
                    )?
                }
                MossSamplingMode::Greedy => self.run_local_greedy_frame(&global_hidden, assets)?,
            };
            if !fixed.should_continue {
                break;
            }
            for (channel, token) in fixed.frame.iter().enumerate() {
                if let Some(seen) = previous_token_sets
                    .get_mut(channel)
                    .and_then(|row| row.get_mut(*token as usize))
                {
                    *seen = true;
                }
            }
            let frame = fixed.frame;
            let mut decode_outputs = self.run_decode_step(&frame, past_valid_length, decode_past, assets)?;
            global_hidden = take_last_hidden_output(&mut decode_outputs, "global_hidden", "tts_decode_step")?;
            decode_past = take_decode_present_outputs(&mut decode_outputs, assets, "tts_decode_step")?;
            drop(decode_outputs);
            frames.push(frame);
        }

        if frames.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "tts_generate_frames",
                detail: "MOSS generated no audio frames".to_string(),
            });
        }
        Ok(frames)
    }

    fn run_local_fixed_sampled_frame(
        &mut self,
        global_hidden: &DynValue,
        previous_token_sets: &[Vec<bool>],
        rng: &mut SimpleRng,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let n_vq = assets.n_vq();
        let codebook_size = assets.audio_codebook_size();
        let mut seen = vec![0i32; n_vq * codebook_size];
        for (channel, tokens) in previous_token_sets.iter().enumerate() {
            for (token, token_seen) in tokens.iter().enumerate() {
                if *token_seen {
                    seen[channel * codebook_size + token] = 1;
                }
            }
        }
        let repetition_seen_mask = Array3::from_shape_vec((1, n_vq, codebook_size), seen)
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;
        let assistant_random_u = Array1::from_vec(vec![rng.next_f32()]);
        let audio_random_u = Array2::from_shape_vec((1, n_vq), (0..n_vq).map(|_| rng.next_f32()).collect())
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;

        let outputs = self
            .local_fixed_sampled_frame
            .run(ort::inputs![
                "global_hidden" => global_hidden,
                "repetition_seen_mask" => TensorRef::from_array_view(repetition_seen_mask.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?,
                "assistant_random_u" => TensorRef::from_array_view(assistant_random_u.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?,
                "audio_random_u" => TensorRef::from_array_view(audio_random_u.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_fixed_sampled_frame", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: e.to_string(),
            })?;
        let should_continue = extract_first_i64(&outputs, "should_continue", "tts_local_fixed_sampled_frame")? > 0;
        let frame = extract_i64_tensor(&outputs, "frame_token_ids", "tts_local_fixed_sampled_frame")?;
        if frame.len() != n_vq {
            return Err(MossTtsError::Inference {
                stage: "tts_local_fixed_sampled_frame",
                detail: format!("expected {n_vq} frame tokens, got {}", frame.len()),
            });
        }
        Ok(GeneratedFrame { should_continue, frame })
    }

    fn run_local_greedy_frame(
        &mut self,
        global_hidden: &DynValue,
        assets: &MossAssets,
    ) -> Result<GeneratedFrame, MossTtsError> {
        let text_token_id = Array1::from_vec(vec![to_i32(
            assets.manifest.tts_config.audio_assistant_slot_token_id,
        )?]);
        let audio_prefix_token_ids =
            Array2::from_elem((1, assets.n_vq().saturating_sub(1)), to_i32(
                assets.manifest.tts_config.audio_pad_token_id,
            )?);
        let outputs = self
            .local_decoder
            .run(ort::inputs![
                "global_hidden" => global_hidden,
                "text_token_id" => TensorRef::from_array_view(text_token_id.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_greedy", detail: e.to_string() })?,
                "audio_prefix_token_ids" => TensorRef::from_array_view(audio_prefix_token_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_local_greedy", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_local_greedy",
                detail: e.to_string(),
            })?;
        let logits = extract_f32_tensor(&outputs, "audio_logits", "tts_local_greedy")?;
        let n_vq = assets.n_vq();
        let codebook_size = assets.audio_codebook_size();
        let frame = greedy_frame_from_logits(&logits, n_vq, codebook_size)?;
        Ok(GeneratedFrame {
            should_continue: true,
            frame,
        })
    }

    fn run_decode_step<'a>(
        &'a mut self,
        frame: &[i64],
        past_valid_length: i64,
        decode_past: Vec<(String, DynValue)>,
        assets: &MossAssets,
    ) -> Result<ort::session::SessionOutputs<'a>, MossTtsError> {
        let mut row = vec![assets.manifest.tts_config.audio_pad_token_id; assets.row_width()];
        row[0] = assets.manifest.tts_config.audio_assistant_slot_token_id;
        for (index, token) in frame.iter().take(assets.n_vq()).enumerate() {
            row[index + 1] = *token;
        }
        let input_ids = Array3::from_shape_vec(
            (1, 1, assets.row_width()),
            row.into_iter().map(to_i32).collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })?;
        let past_valid_lengths = Array1::from_vec(vec![to_i32(past_valid_length)?]);
        let mut inputs = ort::inputs![
            "input_ids" => TensorRef::from_array_view(input_ids.view()).map_err(|e| MossTtsError::Inference { stage: "tts_decode_step", detail: e.to_string() })?,
            "past_valid_lengths" => TensorRef::from_array_view(past_valid_lengths.view()).map_err(|e| MossTtsError::Inference { stage: "tts_decode_step", detail: e.to_string() })?
        ];
        for (name, value) in decode_past {
            inputs.push((name.into(), SessionInputValue::from(value)));
        }
        self.decode_step.run(inputs).map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })
    }

    #[allow(dead_code)]
    fn decode_full(&mut self, audio_frames: Vec<Vec<i64>>) -> Result<TtsResult, MossTtsError> {
        let frames = audio_frames.len();
        let quantizers = audio_frames.first().map(Vec::len).unwrap_or(0);
        let audio_codes = audio_frames
            .into_iter()
            .flatten()
            .map(to_i32)
            .collect::<Result<Vec<_>, _>>()?;
        let codes = Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
            MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            }
        })?;
        let lengths = Array1::from_vec(vec![to_i32(frames as i64)?]);
        let lengths = lengths.into_shape_clone((1,)).map_err(|e| {
            MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            }
        })?;

        let outputs = self
            .codec_decode_full
            .run(ort::inputs![
                "audio_codes" => TensorRef::from_array_view(codes.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?,
                "audio_code_lengths" => TensorRef::from_array_view(lengths.view()).map_err(|e| MossTtsError::Inference { stage: "codec_decode_full", detail: e.to_string() })?
            ])
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            })?;
        let (audio_shape, audio_data) = outputs
            .get("audio")
            .ok_or_else(|| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: "missing audio output".to_string(),
            })?
            .try_extract_tensor::<f32>()
            .map_err(|e| MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            })?;
        let audio_len = outputs
            .get("audio_lengths")
            .and_then(|value| value.try_extract_tensor::<i64>().ok())
            .and_then(|(_, data)| data.first().copied())
            .map(|len| len.max(0) as usize)
            .unwrap_or(audio_shape[2] as usize)
            .min(audio_shape[2] as usize);
        let samples =
            interleave_codec_audio(audio_shape, audio_data, audio_len, "codec_decode_full")?;
        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(samples),
            },
        })
    }
}

fn interleave_codec_audio(
    audio_shape: &[i64],
    audio_data: &[f32],
    audio_len: usize,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    if audio_shape.len() != 3 || audio_shape[0] != 1 || audio_shape[1] != PLAYBACK_CHANNELS as i64 {
        return Err(MossTtsError::OutputFormat(format!(
            "expected codec audio shape [1, {}, samples], got {:?}",
            PLAYBACK_CHANNELS, audio_shape
        )));
    }
    let total_samples = audio_shape[2].max(0) as usize;
    let channels = PLAYBACK_CHANNELS as usize;
    let expected = total_samples * channels;
    if audio_data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("codec audio data too short: expected {expected}, got {}", audio_data.len()),
        });
    }
    let audio_len = audio_len.min(total_samples);
    let mut samples = Vec::with_capacity(audio_len * channels);
    for sample_index in 0..audio_len {
        for channel_index in 0..channels {
            samples.push(audio_data[channel_index * total_samples + sample_index]);
        }
    }
    Ok(samples)
}

fn extract_codec_audio_chunk(
    outputs: &ort::session::SessionOutputs<'_>,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    let (audio_shape, audio_data) = outputs
        .get("audio")
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "missing audio output".to_string(),
        })?
        .try_extract_tensor::<f32>()
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })?;
    let audio_len = outputs
        .get("audio_lengths")
        .and_then(|value| value.try_extract_tensor::<i64>().ok())
        .and_then(|(_, data)| data.first().copied())
        .map(|len| len.max(0) as usize)
        .unwrap_or(audio_shape[2] as usize)
        .min(audio_shape[2] as usize);
    interleave_codec_audio(audio_shape, audio_data, audio_len, stage)
}

fn collect_codec_decode_state_outputs(
    outputs: &ort::session::SessionOutputs<'_>,
    state: &CodecDecodeStepState,
) -> Result<HashMap<String, OwnedTensorData>, MossTtsError> {
    let mut values = HashMap::new();
    for offset in &state.transformer_offsets {
        values.insert(
            offset.output_name.clone(),
            extract_owned_i32_tensor(outputs, &offset.output_name, "codec_decode_step")?,
        );
    }
    for cache in &state.attention_caches {
        values.insert(
            cache.offset.output_name.clone(),
            extract_owned_i32_tensor(outputs, &cache.offset.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.keys.output_name.clone(),
            extract_owned_f32_tensor(outputs, &cache.keys.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.values.output_name.clone(),
            extract_owned_f32_tensor(outputs, &cache.values.output_name, "codec_decode_step")?,
        );
        values.insert(
            cache.positions.output_name.clone(),
            extract_owned_i32_tensor(outputs, &cache.positions.output_name, "codec_decode_step")?,
        );
    }
    Ok(values)
}

fn extract_owned_i32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<OwnedTensorData, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (shape, data) = value.try_extract_tensor::<i32>().map_err(|e| {
        MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        }
    })?;
    Ok(OwnedTensorData::I32 {
        shape: shape.iter().map(|dim| (*dim).max(0) as usize).collect(),
        data: data.to_vec(),
    })
}

fn extract_owned_f32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<OwnedTensorData, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| {
        MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        }
    })?;
    Ok(OwnedTensorData::F32 {
        shape: shape.iter().map(|dim| (*dim).max(0) as usize).collect(),
        data: data.to_vec(),
    })
}

fn create_session(path: &Path, model_name: &'static str) -> Result<Session, MossTtsError> {
    Session::builder()
        .and_then(|b| {
            b.with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(0)?
                .commit_from_file(path)
        })
        .map_err(|e| MossTtsError::Inference {
            stage: "session_init",
            detail: format!("{model_name}: {e}"),
        })
}

fn validate_session_io<I, O>(
    session: &Session,
    model_name: &'static str,
    expected_inputs: I,
    expected_outputs: O,
) -> Result<(), MossTtsError>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
    O: IntoIterator,
    O::Item: AsRef<str>,
{
    for name in expected_inputs {
        let name = name.as_ref();
        if !session.inputs().iter().any(|input| input.name() == name) {
            return Err(MossTtsError::MetadataMismatch(format!(
                "{model_name} missing input '{name}'"
            )));
        }
    }
    for name in expected_outputs {
        let name = name.as_ref();
        if !session.outputs().iter().any(|output| output.name() == name) {
            return Err(MossTtsError::MetadataMismatch(format!(
                "{model_name} missing output '{name}'"
            )));
        }
    }
    Ok(())
}

fn required_file<'a>(files: &'a HashMap<String, PathBuf>, key: &str) -> Result<&'a Path, MossTtsError> {
    files
        .get(key)
        .map(PathBuf::as_path)
        .ok_or_else(|| MossTtsError::MetadataMismatch(format!("missing model file key '{key}'")))
}

fn to_i32(value: i64) -> Result<i32, MossTtsError> {
    i32::try_from(value).map_err(|_| MossTtsError::Inference {
        stage: "tensor_cast",
        detail: format!("value {value} does not fit in int32 tensor input"),
    })
}

fn validate_meta_consistency(
    manifest: &Manifest,
    tts_meta: &TtsMeta,
    codec_meta: &CodecMeta,
) -> Result<(), MossTtsError> {
    if manifest.format_version == 0 || tts_meta.format_version == 0 || codec_meta.format_version == 0 {
        return Err(MossTtsError::MetadataMismatch(
            "format_version must be greater than zero".to_string(),
        ));
    }
    if manifest.tts_config.n_vq != codec_meta.codec_config.num_quantizers {
        return Err(MossTtsError::MetadataMismatch(format!(
            "manifest n_vq {} != codec num_quantizers {}",
            manifest.tts_config.n_vq, codec_meta.codec_config.num_quantizers
        )));
    }
    if codec_meta.codec_config.sample_rate != PLAYBACK_SAMPLE_RATE_HZ {
        return Err(MossTtsError::OutputFormat(format!(
            "codec sample_rate must be {}, got {}",
            PLAYBACK_SAMPLE_RATE_HZ, codec_meta.codec_config.sample_rate
        )));
    }
    if codec_meta.codec_config.channels != PLAYBACK_CHANNELS {
        return Err(MossTtsError::OutputFormat(format!(
            "codec channels must be {}, got {}",
            PLAYBACK_CHANNELS, codec_meta.codec_config.channels
        )));
    }
    Ok(())
}

fn validate_model_files(
    base_dir: &Path,
    prefix: &'static str,
    files: &HashMap<String, String>,
    external_data_files: &HashMap<String, Vec<String>>,
) -> Result<HashMap<String, PathBuf>, MossTtsError> {
    let mut resolved = HashMap::new();
    for (key, raw_path) in files {
        let path = resolve_manifest_path(base_dir, &format!("{prefix}.files.{key}"), raw_path)?;
        ensure_file(&path, "onnx")?;
        if let Some(external_files) = external_data_files.get(raw_path) {
            for external in external_files {
                let external_path = resolve_manifest_path(
                    base_dir,
                    &format!("{prefix}.external_data_files.{raw_path}"),
                    external,
                )?;
                ensure_file(&external_path, "external data")?;
            }
        }
        resolved.insert(key.clone(), path);
    }
    Ok(resolved)
}

fn resolve_manifest_path(base_dir: &Path, field: &str, raw: &str) -> Result<PathBuf, MossTtsError> {
    let path = Path::new(raw);
    if path.is_absolute() || path.components().any(|c| matches!(c, Component::Prefix(_))) {
        return Err(MossTtsError::InvalidRelativePath {
            field: field.to_string(),
            raw: raw.to_string(),
        });
    }
    Ok(base_dir.join(path))
}

fn ensure_file(path: &Path, kind: &'static str) -> Result<(), MossTtsError> {
    if !path.is_file() {
        return Err(MossTtsError::MissingFile {
            kind,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, MossTtsError> {
    let bytes = std::fs::read(path).map_err(|e| MossTtsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|e| MossTtsError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

struct GeneratedFrame {
    should_continue: bool,
    frame: Vec<i64>,
}

fn take_output(
    outputs: &mut ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<DynValue, MossTtsError> {
    outputs.remove(name).ok_or_else(|| MossTtsError::Inference {
        stage,
        detail: format!("missing output '{name}'"),
    })
}

fn take_last_hidden_output(
    outputs: &mut ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<DynValue, MossTtsError> {
    let value = take_output(outputs, name, stage)?;
    let (shape, data) = value.try_extract_tensor::<f32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    match shape.as_ref() {
        [1, _, hidden_size] => {
            let hidden_size = *hidden_size as usize;
            let seq_len = shape[1] as usize;
            if seq_len == 0 {
                return Err(MossTtsError::Inference {
                    stage,
                    detail: format!("output '{name}' has empty sequence axis"),
                });
            }
            let hidden = Array3::from_shape_vec((1, seq_len, hidden_size), data.to_vec())
                .map_err(|e| MossTtsError::Inference {
                    stage,
                    detail: e.to_string(),
                })?;
            let last_hidden = hidden.slice(s![0..1, seq_len - 1, ..]).to_owned();
            Value::from_array(last_hidden)
                .map(|tensor| tensor.into_dyn())
                .map_err(|e| MossTtsError::Inference {
                    stage,
                    detail: e.to_string(),
                })
        }
        [1, _] => Ok(value),
        other => Err(MossTtsError::Inference {
            stage,
            detail: format!("unexpected output '{name}' shape {other:?}"),
        }),
    }
}

fn take_decode_present_outputs(
    outputs: &mut ort::session::SessionOutputs<'_>,
    assets: &MossAssets,
    stage: &'static str,
) -> Result<Vec<(String, DynValue)>, MossTtsError> {
    let input_names = assets.tts_meta.onnx.decode_input_names.iter().skip(2);
    let output_names = assets.tts_meta.onnx.decode_output_names.iter().skip(1);
    let mut values = Vec::new();
    for (input_name, output_name) in input_names.zip(output_names) {
        values.push((input_name.clone(), take_output(outputs, output_name, stage)?));
    }
    Ok(values)
}

fn extract_i64_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<Vec<i64>, MossTtsError> {
    let value = outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?;
    if let Ok((_, data)) = value.try_extract_tensor::<i64>() {
        return Ok(data.to_vec());
    }
    value
        .try_extract_tensor::<i32>()
        .map(|(_, data)| data.iter().map(|value| *value as i64).collect())
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })
}

fn extract_audio_codes(
    outputs: &ort::session::SessionOutputs<'_>,
    stage: &'static str,
) -> Result<Vec<Vec<i64>>, MossTtsError> {
    let value = outputs
        .get("audio_codes")
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: "missing output 'audio_codes'".to_string(),
        })?;
    let (shape, data) = value.try_extract_tensor::<i32>().map_err(|e| MossTtsError::Inference {
        stage,
        detail: e.to_string(),
    })?;
    if shape.len() != 3 || shape[0] != 1 {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("expected audio_codes shape [1, frames, quantizers], got {shape:?}"),
        });
    }
    let frames = shape[1].max(0) as usize;
    let quantizers = shape[2].max(0) as usize;
    let expected = frames * quantizers;
    if data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("audio_codes data too short: expected {expected}, got {}", data.len()),
        });
    }
    let length = outputs
        .get("audio_code_lengths")
        .and_then(|value| value.try_extract_tensor::<i32>().ok())
        .and_then(|(_, data)| data.first().copied())
        .map(|value| value.max(0) as usize)
        .unwrap_or(frames)
        .min(frames);
    audio_codes_from_flat_data(shape, data, length, stage)
}

fn audio_codes_from_flat_data(
    shape: &[i64],
    data: &[i32],
    length: usize,
    stage: &'static str,
) -> Result<Vec<Vec<i64>>, MossTtsError> {
    if shape.len() != 3 || shape[0] != 1 {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("expected audio_codes shape [1, frames, quantizers], got {shape:?}"),
        });
    }
    let frames = shape[1].max(0) as usize;
    let quantizers = shape[2].max(0) as usize;
    let expected = frames * quantizers;
    if data.len() < expected {
        return Err(MossTtsError::Inference {
            stage,
            detail: format!("audio_codes data too short: expected {expected}, got {}", data.len()),
        });
    }
    let length = length.min(frames);
    let mut codes = Vec::with_capacity(length);
    for frame_index in 0..length {
        let start = frame_index * quantizers;
        codes.push(
            data[start..start + quantizers]
                .iter()
                .map(|value| *value as i64)
                .collect(),
        );
    }
    if codes.is_empty() {
        return Err(MossTtsError::Inference {
            stage,
            detail: "codec encode produced no audio codes".to_string(),
        });
    }
    Ok(codes)
}

fn extract_first_i64(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<i64, MossTtsError> {
    extract_i64_tensor(outputs, name, stage)?
        .first()
        .copied()
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("output '{name}' is empty"),
        })
}

fn extract_f32_tensor(
    outputs: &ort::session::SessionOutputs<'_>,
    name: &str,
    stage: &'static str,
) -> Result<Vec<f32>, MossTtsError> {
    outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?
        .try_extract_tensor::<f32>()
        .map(|(_, data)| data.to_vec())
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })
}

fn greedy_frame_from_logits(
    logits: &[f32],
    n_vq: usize,
    codebook_size: usize,
) -> Result<Vec<i64>, MossTtsError> {
    let expected = n_vq * codebook_size;
    if logits.len() != expected {
        return Err(MossTtsError::Inference {
            stage: "tts_local_greedy",
            detail: format!("expected {expected} logits, got {}", logits.len()),
        });
    }
    let mut frame = Vec::with_capacity(n_vq);
    for channel_logits in logits.chunks_exact(codebook_size) {
        let (token, _) = channel_logits
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.total_cmp(right))
            .ok_or_else(|| MossTtsError::Inference {
                stage: "tts_local_greedy",
                detail: "empty codebook logits".to_string(),
            })?;
        frame.push(token as i64);
    }
    Ok(frame)
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        let state = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self { state }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        let value = ((self.state >> 40) as f32) / ((1u64 << 24) as f32);
        value.clamp(0.0, 0.999_999_94)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    format_version: u32,
    model_files: ManifestModelFiles,
    tts_config: ManifestTtsConfig,
    prompt_templates: PromptTemplates,
    generation_defaults: GenerationDefaults,
    #[serde(default)]
    builtin_voices: Vec<BuiltinVoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestModelFiles {
    tts_meta: String,
    codec_meta: String,
    tokenizer_model: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestTtsConfig {
    n_vq: u32,
    audio_pad_token_id: i64,
    audio_start_token_id: i64,
    audio_end_token_id: i64,
    audio_user_slot_token_id: i64,
    audio_assistant_slot_token_id: i64,
    #[serde(default)]
    audio_codebook_sizes: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct PromptTemplates {
    user_prompt_prefix_token_ids: Vec<i64>,
    user_prompt_after_reference_token_ids: Vec<i64>,
    assistant_prompt_prefix_token_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GenerationDefaults {
    max_new_frames: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct BuiltinVoice {
    voice: String,
    #[serde(default)]
    prompt_audio_codes: Vec<Vec<i64>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TtsMeta {
    format_version: u32,
    files: HashMap<String, String>,
    #[serde(default)]
    external_data_files: HashMap<String, Vec<String>>,
    onnx: TtsOnnxMeta,
}

#[derive(Debug, Clone, Deserialize)]
struct TtsOnnxMeta {
    #[serde(default)]
    prefill_output_names: Vec<String>,
    #[serde(default)]
    decode_input_names: Vec<String>,
    #[serde(default)]
    decode_output_names: Vec<String>,
    #[serde(default)]
    local_cached_input_names: Vec<String>,
    #[serde(default)]
    local_cached_output_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodecMeta {
    format_version: u32,
    files: HashMap<String, String>,
    #[serde(default)]
    external_data_files: HashMap<String, Vec<String>>,
    codec_config: CodecConfig,
    onnx: CodecOnnxMeta,
    #[serde(default)]
    streaming_decode: Option<StreamingDecodeMeta>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodecConfig {
    sample_rate: u32,
    channels: u16,
    num_quantizers: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct CodecOnnxMeta {
    #[serde(default)]
    encode_input_names: Vec<String>,
    #[serde(default)]
    encode_output_names: Vec<String>,
    #[serde(default)]
    decode_input_names: Vec<String>,
    #[serde(default)]
    decode_output_names: Vec<String>,
    #[serde(default)]
    decode_step_input_names: Vec<String>,
    #[serde(default)]
    decode_step_output_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StreamingDecodeMeta {
    #[serde(default = "default_decode_step_batch_size")]
    batch_size: usize,
    #[serde(default)]
    transformer_offsets: Vec<TransformerOffsetMeta>,
    #[serde(default)]
    attention_caches: Vec<AttentionCacheMeta>,
}

#[derive(Debug, Clone, Deserialize)]
struct TransformerOffsetMeta {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    dtype: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AttentionCacheMeta {
    offset_input_name: String,
    offset_output_name: String,
    cached_keys_input_name: String,
    cached_keys_output_name: String,
    cached_values_input_name: String,
    cached_values_output_name: String,
    cached_positions_input_name: String,
    cached_positions_output_name: String,
    offset_shape: Vec<usize>,
    cache_shape: Vec<usize>,
    positions_shape: Vec<usize>,
    cache_dtype: String,
    positions_dtype: String,
}

fn default_decode_step_batch_size() -> usize {
    1
}

#[derive(Debug, Clone)]
struct CodecDecodeStepState {
    batch_size: usize,
    transformer_offsets: Vec<NamedI32TensorState>,
    attention_caches: Vec<AttentionCacheState>,
}

#[derive(Debug, Clone)]
struct NamedI32TensorState {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    data: Vec<i32>,
}

#[derive(Debug, Clone)]
struct NamedF32TensorState {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    data: Vec<f32>,
}

#[derive(Debug, Clone)]
struct AttentionCacheState {
    offset: NamedI32TensorState,
    keys: NamedF32TensorState,
    values: NamedF32TensorState,
    positions: NamedI32TensorState,
}

impl CodecDecodeStepState {
    fn from_meta(meta: &CodecMeta) -> Result<Self, MossTtsError> {
        let streaming = meta.streaming_decode.as_ref().ok_or_else(|| {
            MossTtsError::MetadataMismatch(
                "codec streaming_decode metadata is required for decode_step".to_string(),
            )
        })?;
        if streaming.batch_size == 0 {
            return Err(MossTtsError::MetadataMismatch(
                "codec streaming_decode.batch_size must be greater than zero".to_string(),
            ));
        }

        let transformer_offsets = streaming
            .transformer_offsets
            .iter()
            .map(|offset| {
                ensure_dtype(&offset.dtype, "int32", &offset.input_name)?;
                Ok(NamedI32TensorState::zeros(
                    offset.input_name.clone(),
                    offset.output_name.clone(),
                    offset.shape.clone(),
                ))
            })
            .collect::<Result<Vec<_>, MossTtsError>>()?;

        let attention_caches = streaming
            .attention_caches
            .iter()
            .map(|cache| {
                ensure_dtype("int32", "int32", &cache.offset_input_name)?;
                ensure_dtype(&cache.cache_dtype, "float32", &cache.cached_keys_input_name)?;
                ensure_dtype(&cache.cache_dtype, "float32", &cache.cached_values_input_name)?;
                ensure_dtype(
                    &cache.positions_dtype,
                    "int32",
                    &cache.cached_positions_input_name,
                )?;
                Ok(AttentionCacheState {
                    offset: NamedI32TensorState::zeros(
                        cache.offset_input_name.clone(),
                        cache.offset_output_name.clone(),
                        cache.offset_shape.clone(),
                    ),
                    keys: NamedF32TensorState::zeros(
                        cache.cached_keys_input_name.clone(),
                        cache.cached_keys_output_name.clone(),
                        cache.cache_shape.clone(),
                    ),
                    values: NamedF32TensorState::zeros(
                        cache.cached_values_input_name.clone(),
                        cache.cached_values_output_name.clone(),
                        cache.cache_shape.clone(),
                    ),
                    positions: NamedI32TensorState::zeros(
                        cache.cached_positions_input_name.clone(),
                        cache.cached_positions_output_name.clone(),
                        cache.positions_shape.clone(),
                    ),
                })
            })
            .collect::<Result<Vec<_>, MossTtsError>>()?;

        Ok(Self {
            batch_size: streaming.batch_size,
            transformer_offsets,
            attention_caches,
        })
    }

    #[cfg(test)]
    fn input_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for offset in &self.transformer_offsets {
            names.push(offset.input_name.as_str());
        }
        for cache in &self.attention_caches {
            names.push(cache.offset.input_name.as_str());
            names.push(cache.keys.input_name.as_str());
            names.push(cache.values.input_name.as_str());
            names.push(cache.positions.input_name.as_str());
        }
        names
    }

    #[cfg(test)]
    fn output_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for offset in &self.transformer_offsets {
            names.push(offset.output_name.as_str());
        }
        for cache in &self.attention_caches {
            names.push(cache.offset.output_name.as_str());
            names.push(cache.keys.output_name.as_str());
            names.push(cache.values.output_name.as_str());
            names.push(cache.positions.output_name.as_str());
        }
        names
    }

    fn update_from_owned_outputs(
        &mut self,
        outputs: &HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        for offset in &mut self.transformer_offsets {
            offset.update_from_outputs(outputs)?;
        }
        for cache in &mut self.attention_caches {
            cache.offset.update_from_outputs(outputs)?;
            cache.keys.update_from_outputs(outputs)?;
            cache.values.update_from_outputs(outputs)?;
            cache.positions.update_from_outputs(outputs)?;
        }
        Ok(())
    }

    fn collect_input_arrays(
        &self,
        i32_tensors: &mut Vec<(String, ArrayD<i32>)>,
        f32_tensors: &mut Vec<(String, ArrayD<f32>)>,
    ) -> Result<(), MossTtsError> {
        for offset in &self.transformer_offsets {
            i32_tensors.push((offset.input_name.clone(), offset.to_array()?));
        }
        for cache in &self.attention_caches {
            i32_tensors.push((cache.offset.input_name.clone(), cache.offset.to_array()?));
            f32_tensors.push((cache.keys.input_name.clone(), cache.keys.to_array()?));
            f32_tensors.push((cache.values.input_name.clone(), cache.values.to_array()?));
            i32_tensors.push((cache.positions.input_name.clone(), cache.positions.to_array()?));
        }
        Ok(())
    }
}

impl NamedI32TensorState {
    fn zeros(input_name: String, output_name: String, shape: Vec<usize>) -> Self {
        let len = tensor_len(&shape);
        Self {
            input_name,
            output_name,
            shape,
            data: vec![0; len],
        }
    }

    fn update_from_outputs(
        &mut self,
        outputs: &HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        let tensor = outputs
            .get(&self.output_name)
            .ok_or_else(|| codec_decode_step_unavailable(format!(
                "missing state output '{}'",
                self.output_name
            )))?;
        let OwnedTensorData::I32 { shape, data } = tensor else {
            return Err(codec_decode_step_unavailable(format!(
                "state output '{}' must be int32",
                self.output_name
            )));
        };
        self.shape = shape.clone();
        self.data = data.clone();
        Ok(())
    }

    fn to_array(&self) -> Result<ArrayD<i32>, MossTtsError> {
        ArrayD::from_shape_vec(IxDyn(&self.shape), self.data.clone()).map_err(|e| {
            codec_decode_step_unavailable(format!(
                "failed to build state tensor '{}': {e}",
                self.input_name
            ))
        })
    }
}

impl NamedF32TensorState {
    fn zeros(input_name: String, output_name: String, shape: Vec<usize>) -> Self {
        let len = tensor_len(&shape);
        Self {
            input_name,
            output_name,
            shape,
            data: vec![0.0; len],
        }
    }

    fn update_from_outputs(
        &mut self,
        outputs: &HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        let tensor = outputs
            .get(&self.output_name)
            .ok_or_else(|| codec_decode_step_unavailable(format!(
                "missing state output '{}'",
                self.output_name
            )))?;
        let OwnedTensorData::F32 { shape, data } = tensor else {
            return Err(codec_decode_step_unavailable(format!(
                "state output '{}' must be float32",
                self.output_name
            )));
        };
        self.shape = shape.clone();
        self.data = data.clone();
        Ok(())
    }

    fn to_array(&self) -> Result<ArrayD<f32>, MossTtsError> {
        ArrayD::from_shape_vec(IxDyn(&self.shape), self.data.clone()).map_err(|e| {
            codec_decode_step_unavailable(format!(
                "failed to build state tensor '{}': {e}",
                self.input_name
            ))
        })
    }
}

#[derive(Debug, Clone)]
enum OwnedTensorData {
    I32 { shape: Vec<usize>, data: Vec<i32> },
    F32 { shape: Vec<usize>, data: Vec<f32> },
}

fn tensor_len(shape: &[usize]) -> usize {
    shape.iter().copied().product::<usize>().max(1)
}

fn ensure_dtype(actual: &str, expected: &str, name: &str) -> Result<(), MossTtsError> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(MossTtsError::MetadataMismatch(format!(
            "{name} expected dtype {expected}, got {actual}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tts_core::MossTtsConfig;
    use tempfile::TempDir;

    #[test]
    fn loads_valid_asset_layout() {
        let fixture = MossFixture::new();

        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir.clone(),
        })
        .expect("assets should load");

        assert!(assets.manifest_path.ends_with(MANIFEST_FILE));
        assert!(assets.tts_files.contains_key("prefill"));
        assert!(assets.codec_files.contains_key("decode_full"));
    }

    #[test]
    fn reports_missing_required_file() {
        let fixture = MossFixture::new();
        std::fs::remove_file(fixture.tts_dir.join("tokenizer.model")).unwrap();

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("tokenizer is required");

        assert!(matches!(err, MossTtsError::MissingFile { .. }));
        assert!(err.to_string().contains("tokenizer.model"));
    }

    #[test]
    fn rejects_absolute_manifest_relative_path() {
        let fixture = MossFixture::new();
        fixture.write_manifest("/tmp/tts_meta.json", "../codec/codec_browser_onnx_meta.json", "tokenizer.model");

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("absolute path should fail");

        assert!(matches!(err, MossTtsError::InvalidRelativePath { .. }));
        assert!(err.to_string().contains("model_files.tts_meta"));
    }

    #[test]
    fn rejects_codec_contract_mismatch() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta(44_100, 1, 16);

        let err = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .expect_err("codec contract mismatch should fail");

        assert!(matches!(err, MossTtsError::OutputFormat(_)));
        assert!(err.to_string().contains("48000"));
    }

    #[test]
    fn maps_default_and_named_voice() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        assert_eq!(assets.resolve_voice(None).unwrap().voice, DEFAULT_VOICE);
        assert_eq!(assets.resolve_voice(Some("ava")).unwrap().voice, "Ava");
    }

    #[test]
    fn rejects_unknown_voice_with_available_list() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = assets.resolve_voice(Some("Nobody")).expect_err("unknown voice should fail");

        assert!(matches!(err, MossTtsError::UnknownVoice { .. }));
        assert!(err.to_string().contains("Junhao"));
    }

    #[test]
    fn sampling_mode_defaults_to_fixed() {
        assert_eq!(
            MossSamplingMode::from_config(&TtsConfig::default()).unwrap(),
            MossSamplingMode::Fixed
        );
    }

    #[test]
    fn sampling_mode_accepts_fixed_and_greedy() {
        let fixed = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("fixed".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };
        let greedy = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("Greedy".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };

        assert_eq!(
            MossSamplingMode::from_config(&fixed).unwrap(),
            MossSamplingMode::Fixed
        );
        assert_eq!(
            MossSamplingMode::from_config(&greedy).unwrap(),
            MossSamplingMode::Greedy
        );
    }

    #[test]
    fn sampling_mode_rejects_unknown_mode_with_available_list() {
        let config = TtsConfig {
            moss: Some(MossTtsConfig {
                sampling_mode: Some("creative".to_string()),
                ..MossTtsConfig::default()
            }),
            ..TtsConfig::default()
        };

        let err = MossSamplingMode::from_config(&config).unwrap_err();

        assert!(matches!(err, MossTtsError::UnknownSamplingMode { .. }));
        assert!(err.to_string().contains("creative"));
        assert!(err.to_string().contains("fixed"));
        assert!(err.to_string().contains("greedy"));
    }

    #[test]
    fn greedy_frame_selects_deterministic_argmax_per_codebook() {
        let logits = vec![
            0.1, 0.7, 0.2,
            4.0, -1.0, 3.0,
        ];

        let first = greedy_frame_from_logits(&logits, 2, 3).unwrap();
        let second = greedy_frame_from_logits(&logits, 2, 3).unwrap();

        assert_eq!(first, vec![1, 0]);
        assert_eq!(first, second);
    }

    #[test]
    fn greedy_frame_rejects_unexpected_logits_shape() {
        let err = greedy_frame_from_logits(&[0.0, 1.0], 2, 3).unwrap_err();

        assert!(err.to_string().contains("expected 6 logits"));
    }

    #[test]
    fn enforces_output_contract() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let engine = MossOnnxTtsEngine::from_assets_for_test(assets);

        let result = TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: 44_100,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(vec![0.0; 2]),
            },
        };

        let err = engine
            .validate_output_contract(&result)
            .expect_err("sample rate must match playback contract");
        assert!(matches!(err, MossTtsError::OutputFormat(_)));
    }

    #[test]
    fn builds_prompt_rows_from_prepared_chunk_tokens() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let voice = assets.resolve_voice(None).unwrap();

        let request = assets
            .build_voice_clone_request_rows([101, 102, 103], &voice.prompt_audio_codes)
            .unwrap();

        assert_eq!(request.attention_mask, vec![1; request.rows.len()]);
        assert!(request
            .rows
            .iter()
            .any(|row| row.first() == Some(&101)));
        assert!(request
            .rows
            .iter()
            .any(|row| row.first() == Some(&assets.manifest.tts_config.audio_start_token_id)));
    }

    #[test]
    fn reference_prompt_codes_replace_builtin_voice_codes() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let voice = assets.resolve_voice(None).unwrap();
        let reference_codes = vec![vec![99; assets.n_vq()]];

        let request = assets
            .build_voice_clone_request_rows([101], &reference_codes)
            .unwrap();

        assert!(request.rows.iter().any(|row| row[1] == 99));
        assert!(!request
            .rows
            .iter()
            .any(|row| row.get(1) == voice.prompt_audio_codes[0].first()));
    }

    #[test]
    fn codec_encode_codes_preserve_shape_and_length() {
        let codes =
            audio_codes_from_flat_data(&[1, 2, 3], &[1, 2, 3, 4, 5, 6], 1, "codec_encode")
                .unwrap();

        assert_eq!(codes, vec![vec![1, 2, 3]]);
    }

    #[test]
    fn codec_encode_codes_reject_invalid_shape() {
        let err = audio_codes_from_flat_data(&[2, 2, 3], &[0; 12], 2, "codec_encode")
            .expect_err("batch must be one");

        assert!(err.to_string().contains("codec_encode"));
    }

    #[test]
    fn decode_step_state_initializes_from_streaming_metadata() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();

        assert_eq!(state.batch_size, 1);
        assert_eq!(state.transformer_offsets.len(), 1);
        assert_eq!(state.attention_caches.len(), 1);
        assert_eq!(state.transformer_offsets[0].shape, vec![1]);
        assert_eq!(state.transformer_offsets[0].data, vec![0]);
        assert_eq!(state.attention_caches[0].keys.shape, vec![1, 4, 8, 64]);
        assert_eq!(state.attention_caches[0].keys.data.len(), 1 * 4 * 8 * 64);
        assert!(state.input_names().contains(&"transformer_offset_0"));
        assert!(state.output_names().contains(&"attn_cached_values_out_0"));
    }

    #[test]
    fn decode_step_state_rejects_missing_streaming_metadata() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta_without_streaming(48_000, 2, 16);
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap_err();

        assert!(err.to_string().contains("streaming_decode"));
    }

    #[test]
    fn decode_step_state_rejects_bad_cache_dtype() {
        let fixture = MossFixture::new();
        fixture.write_codec_meta_with_cache_dtype(48_000, 2, 16, "float16");
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();

        let err = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap_err();

        assert!(err.to_string().contains("float32"));
        assert!(err.to_string().contains("float16"));
    }

    #[test]
    fn decode_step_state_updates_from_owned_outputs() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let mut state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();
        let mut outputs = HashMap::new();
        outputs.insert(
            "transformer_offset_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1],
                data: vec![3],
            },
        );
        outputs.insert(
            "attn_offset_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1],
                data: vec![4],
            },
        );
        outputs.insert(
            "attn_cached_keys_out_0".to_string(),
            OwnedTensorData::F32 {
                shape: vec![1, 4, 2, 64],
                data: vec![0.5; 1 * 4 * 2 * 64],
            },
        );
        outputs.insert(
            "attn_cached_values_out_0".to_string(),
            OwnedTensorData::F32 {
                shape: vec![1, 4, 2, 64],
                data: vec![0.25; 1 * 4 * 2 * 64],
            },
        );
        outputs.insert(
            "attn_cached_positions_out_0".to_string(),
            OwnedTensorData::I32 {
                shape: vec![1, 2],
                data: vec![1, 2],
            },
        );

        state.update_from_owned_outputs(&outputs).unwrap();

        assert_eq!(state.transformer_offsets[0].data, vec![3]);
        assert_eq!(state.attention_caches[0].offset.data, vec![4]);
        assert_eq!(state.attention_caches[0].keys.shape, vec![1, 4, 2, 64]);
        assert_eq!(state.attention_caches[0].positions.data, vec![1, 2]);
    }

    #[test]
    fn decode_step_state_update_rejects_missing_output_name() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let mut state = CodecDecodeStepState::from_meta(&assets.codec_meta).unwrap();

        let err = state.update_from_owned_outputs(&HashMap::new()).unwrap_err();

        assert!(err.to_string().contains("codec_decode_step"));
        assert!(err.to_string().contains("transformer_offset_out_0"));
    }

    #[test]
    fn decode_step_buffered_reports_stage_when_unavailable() {
        let err = codec_decode_step_unavailable("fallback".to_string());

        assert!(err.to_string().contains("codec_decode_step"));
    }

    #[test]
    fn pcm_chunk_buffer_concatenates_chunks_to_playback_result() {
        let mut buffer = PcmChunkBuffer::default();
        buffer.push_chunk(vec![0.1, 0.2]);
        buffer.push_chunk(Vec::new());
        buffer.push_chunk(vec![0.3, 0.4]);

        let result = buffer.into_tts_result().unwrap();

        assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);
        assert!(matches!(
            result.audio.pcm,
            PcmData::F32(samples) if samples == vec![0.1, 0.2, 0.3, 0.4]
        ));
    }

    #[test]
    fn pcm_chunk_buffer_rejects_unaligned_pcm() {
        let mut buffer = PcmChunkBuffer::default();
        buffer.push_chunk(vec![0.1, 0.2, 0.3]);

        let err = buffer.into_tts_result().unwrap_err();

        assert!(err.to_string().contains("stereo"));
    }

    #[test]
    fn interleaves_codec_audio_from_channel_major_output() {
        let samples = interleave_codec_audio(
            &[1, 2, 3],
            &[0.1, 0.2, 0.3, 1.1, 1.2, 1.3],
            2,
            "codec_decode_step",
        )
        .unwrap();

        assert_eq!(samples, vec![0.1, 1.1, 0.2, 1.2]);
    }

    #[test]
    fn concatenates_multiple_chunk_results_in_order() {
        let result = concatenate_tts_results(vec![
            TtsResult {
                audio: AudioBuffer {
                    sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                    channels: PLAYBACK_CHANNELS,
                    pcm: PcmData::F32(vec![0.1, 0.2, 0.3, 0.4]),
                },
            },
            TtsResult {
                audio: AudioBuffer {
                    sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                    channels: PLAYBACK_CHANNELS,
                    pcm: PcmData::F32(vec![0.5, 0.6]),
                },
            },
        ])
        .unwrap();

        assert_eq!(result.audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(result.audio.channels, PLAYBACK_CHANNELS);
        assert!(matches!(
            result.audio.pcm,
            PcmData::F32(samples) if samples == vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]
        ));
    }

    #[test]
    fn rejects_empty_multi_chunk_concat() {
        let err = concatenate_tts_results(Vec::new()).expect_err("empty chunks should fail");

        assert!(err.to_string().contains("no audio"));
    }

    #[test]
    fn blocking_worker_path_propagates_session_init_errors() {
        let fixture = MossFixture::new();
        let assets = MossAssets::load(MossModelConfig {
            model_dir: fixture.tts_dir,
        })
        .unwrap();
        let sessions = Arc::new(Mutex::new(None));
        let prepared = PreparedSynthesis {
            assets,
            prompt_audio_codes: vec![vec![1; 16]],
            chunks: vec![PreparedTextChunk {
                text: "hello".to_string(),
                token_ids: vec![1, 2, 3],
            }],
            sampling_mode: MossSamplingMode::Fixed,
            reference_audio: None,
        };

        let err = synthesize_prepared_with_sessions(&sessions, prepared)
            .expect_err("invalid fixture ONNX files must surface as worker init errors");

        assert!(err.to_string().contains("session_init"));
    }

    struct MossFixture {
        _temp: TempDir,
        tts_dir: PathBuf,
        codec_dir: PathBuf,
    }

    impl MossFixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let tts_dir = temp.path().join("tts");
            let codec_dir = temp.path().join("codec");
            std::fs::create_dir_all(&tts_dir).unwrap();
            std::fs::create_dir_all(&codec_dir).unwrap();
            for file in [
                "tokenizer.model",
                "prefill.onnx",
                "decode_step.onnx",
                "local_decoder.onnx",
                "local_cached_step.onnx",
                "local_fixed_sampled_frame.onnx",
                "tts_shared.data",
            ] {
                std::fs::write(tts_dir.join(file), b"x").unwrap();
            }
            for file in [
                "encode.onnx",
                "decode_full.onnx",
                "decode_step.onnx",
                "codec_shared.data",
            ] {
                std::fs::write(codec_dir.join(file), b"x").unwrap();
            }
            let fixture = Self {
                _temp: temp,
                tts_dir,
                codec_dir,
            };
            fixture.write_manifest("tts_browser_onnx_meta.json", "../codec/codec_browser_onnx_meta.json", "tokenizer.model");
            fixture.write_tts_meta();
            fixture.write_codec_meta(48_000, 2, 16);
            fixture
        }

    fn write_manifest(&self, tts_meta: &str, codec_meta: &str, tokenizer: &str) {
            std::fs::write(
                self.tts_dir.join(MANIFEST_FILE),
                format!(
                    r#"{{
  "format_version": 1,
  "model_files": {{
    "tts_meta": "{tts_meta}",
    "codec_meta": "{codec_meta}",
    "tokenizer_model": "{tokenizer}"
  }},
  "tts_config": {{
    "n_vq": 16,
    "audio_pad_token_id": 1024,
    "audio_start_token_id": 6,
    "audio_end_token_id": 7,
    "audio_user_slot_token_id": 8,
    "audio_assistant_slot_token_id": 9,
    "audio_codebook_sizes": [1024]
  }},
  "prompt_templates": {{
    "user_prompt_prefix_token_ids": [1, 2],
    "user_prompt_after_reference_token_ids": [3, 4],
    "assistant_prompt_prefix_token_ids": [5]
  }},
  "generation_defaults": {{ "max_new_frames": 4 }},
  "builtin_voices": [
    {{ "voice": "Junhao", "prompt_audio_codes": [[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]] }},
    {{ "voice": "Ava", "prompt_audio_codes": [[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]] }}
  ]
}}"#
                ),
            )
            .unwrap();
        }

        fn write_tts_meta(&self) {
            std::fs::write(
                self.tts_dir.join("tts_browser_onnx_meta.json"),
                r#"{
  "format_version": 1,
  "files": {
    "prefill": "prefill.onnx",
    "decode_step": "decode_step.onnx",
    "local_decoder": "local_decoder.onnx",
    "local_cached_step": "local_cached_step.onnx",
    "local_fixed_sampled_frame": "local_fixed_sampled_frame.onnx"
  },
  "external_data_files": {
    "prefill.onnx": ["tts_shared.data"],
    "decode_step.onnx": ["tts_shared.data"]
  },
  "onnx": {
    "prefill_output_names": ["global_hidden"],
    "decode_input_names": ["input_ids"],
    "decode_output_names": ["global_hidden"],
    "local_cached_input_names": ["global_hidden", "text_token_id"],
    "local_cached_output_names": ["text_logits", "audio_logits"]
  }
}"#,
            )
            .unwrap();
        }

        fn write_codec_meta(&self, sample_rate: u32, channels: u16, num_quantizers: u32) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, Some("float32"));
        }

        fn write_codec_meta_without_streaming(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
        ) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, None);
        }

        fn write_codec_meta_with_cache_dtype(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
            cache_dtype: &str,
        ) {
            self.write_codec_meta_inner(sample_rate, channels, num_quantizers, Some(cache_dtype));
        }

        fn write_codec_meta_inner(
            &self,
            sample_rate: u32,
            channels: u16,
            num_quantizers: u32,
            streaming_cache_dtype: Option<&str>,
        ) {
            let streaming_decode = streaming_cache_dtype
                .map(|cache_dtype| {
                    format!(
                        r#",
  "streaming_decode": {{
    "batch_size": 1,
    "transformer_offsets": [
      {{
        "input_name": "transformer_offset_0",
        "output_name": "transformer_offset_out_0",
        "shape": [1],
        "dtype": "int32"
      }}
    ],
    "attention_caches": [
      {{
        "offset_input_name": "attn_offset_0",
        "offset_output_name": "attn_offset_out_0",
        "cached_keys_input_name": "attn_cached_keys_0",
        "cached_keys_output_name": "attn_cached_keys_out_0",
        "cached_values_input_name": "attn_cached_values_0",
        "cached_values_output_name": "attn_cached_values_out_0",
        "cached_positions_input_name": "attn_cached_positions_0",
        "cached_positions_output_name": "attn_cached_positions_out_0",
        "offset_shape": [1],
        "cache_shape": [1, 4, 8, 64],
        "positions_shape": [1, 8],
        "cache_dtype": "{cache_dtype}",
        "positions_dtype": "int32"
      }}
    ]
  }}"#
                    )
                })
                .unwrap_or_default();
            std::fs::write(
                self.codec_dir.join("codec_browser_onnx_meta.json"),
                format!(
                    r#"{{
  "format_version": 2,
  "files": {{ "encode": "encode.onnx", "decode_full": "decode_full.onnx", "decode_step": "decode_step.onnx" }},
  "external_data_files": {{
    "encode.onnx": ["codec_shared.data"],
    "decode_full.onnx": ["codec_shared.data"],
    "decode_step.onnx": ["codec_shared.data"]
  }},
  "codec_config": {{ "sample_rate": {sample_rate}, "channels": {channels}, "num_quantizers": {num_quantizers} }},
  "onnx": {{
    "encode_input_names": ["waveform", "input_lengths"],
    "encode_output_names": ["audio_codes", "audio_code_lengths"],
    "decode_input_names": ["audio_codes", "audio_code_lengths"],
    "decode_output_names": ["audio", "audio_lengths"],
    "decode_step_input_names": ["audio_codes", "audio_code_lengths"],
    "decode_step_output_names": ["audio", "audio_lengths"]
  }}{streaming_decode}
}}"#
                ),
            )
            .unwrap();
        }
    }
}
