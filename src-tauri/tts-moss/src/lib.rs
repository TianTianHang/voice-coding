use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ndarray::{Array1, Array2, Array3};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::session::SessionInputValue;
use ort::value::{DynValue, TensorRef};
use sentencepiece::SentencePieceProcessor;
use serde::Deserialize;
use tts_core::{
    AudioBuffer, PcmData, TtsConfig, TtsEngine, TtsError, TtsResult, PLAYBACK_CHANNELS,
    PLAYBACK_SAMPLE_RATE_HZ,
};

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
            MossTtsError::UnknownVoice { .. } => TtsError::UnsupportedConfig(value.to_string()),
            MossTtsError::OutputFormat(_) => TtsError::UnsupportedConfig(value.to_string()),
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
        voice: &BuiltinVoice,
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
        for codes in &voice.prompt_audio_codes {
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
    sessions: Mutex<Option<MossSessions>>,
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
            sessions: Mutex::new(None),
        })
    }

    fn ensure_sessions(&self) -> Result<(), MossTtsError> {
        let mut sessions = self.sessions.lock().map_err(|e| MossTtsError::Inference {
            stage: "session_lock",
            detail: e.to_string(),
        })?;
        if sessions.is_none() {
            *sessions = Some(MossSessions::load(&self.assets)?);
        }
        Ok(())
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
            sessions: Mutex::new(None),
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
        let voice = self.assets.resolve_voice(config.voice.as_deref())?;
        self.ensure_sessions()?;
        let text_tokens = self.tokenizer.encode_ids(text)?;
        let request = self.assets.build_voice_clone_request_rows(text_tokens, voice)?;
        let mut sessions = self.sessions.lock().map_err(|e| MossTtsError::Inference {
            stage: "session_lock",
            detail: e.to_string(),
        })?;
        let sessions = sessions.as_mut().ok_or_else(|| MossTtsError::Inference {
            stage: "session_init",
            detail: "MOSS sessions were not initialized".to_string(),
        })?;
        let result = sessions.synthesize(&self.assets, request)?;
        self.validate_output_contract(&result)?;
        Ok(result)
    }

    async fn health_check(&self) -> tts_core::Result<bool> {
        self.ensure_sessions()?;
        Ok(true)
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
    codec_decode_full: Session,
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

        validate_session_io(
            &prefill,
            "tts.prefill",
            &[],
            &assets.tts_meta.onnx.prefill_output_names,
        )?;
        validate_session_io(
            &decode_step,
            "tts.decode_step",
            &assets.tts_meta.onnx.decode_input_names,
            &[],
        )?;
        validate_session_io(
            &codec_decode_full,
            "codec.decode_full",
            &assets.codec_meta.onnx.decode_input_names,
            &assets.codec_meta.onnx.decode_output_names,
        )?;

        Ok(Self {
            prefill,
            decode_step,
            local_decoder,
            local_cached_step,
            local_fixed_sampled_frame,
            codec_decode_full,
        })
    }

    fn synthesize(&mut self, assets: &MossAssets, request: MossRequestRows) -> Result<TtsResult, MossTtsError> {
        let generated_frames = self.generate_audio_frames(assets, request)?;
        self.decode_full(generated_frames)
    }

    fn generate_audio_frames(
        &mut self,
        assets: &MossAssets,
        request: MossRequestRows,
    ) -> Result<Vec<Vec<i64>>, MossTtsError> {
        let row_width = assets.row_width();
        let input_ids = Array3::from_shape_vec((1, request.rows.len(), row_width), request.rows.concat())
            .map_err(|e| MossTtsError::Inference {
                stage: "tts_prefill",
                detail: e.to_string(),
            })?;
        let attention_mask = Array2::from_shape_vec((1, request.attention_mask.len()), request.attention_mask)
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

        let mut global_hidden = take_output(&mut outputs, "global_hidden", "tts_prefill")?;
        let mut decode_past = take_decode_present_outputs(&mut outputs, assets, "tts_prefill")?;
        let initial_past_valid_length = input_ids.shape()[1] as i64;
        drop(outputs);

        let mut frames = Vec::new();
        let mut previous_token_sets = vec![vec![false; assets.audio_codebook_size()]; assets.n_vq()];
        let mut rng = SimpleRng::new();

        for past_valid_length in (initial_past_valid_length..).take(assets.max_new_frames()) {
            let fixed = self.run_local_fixed_sampled_frame(&global_hidden, &previous_token_sets, &mut rng, assets)?;
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
            global_hidden = take_output(&mut decode_outputs, "global_hidden", "tts_decode_step")?;
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
        let mut seen = vec![0i64; n_vq * codebook_size];
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
        let input_ids = Array3::from_shape_vec((1, 1, assets.row_width()), row).map_err(|e| MossTtsError::Inference {
            stage: "tts_decode_step",
            detail: e.to_string(),
        })?;
        let past_valid_lengths = Array1::from_vec(vec![past_valid_length]);
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
        let audio_codes = audio_frames.into_iter().flatten().collect::<Vec<_>>();
        let codes = Array3::from_shape_vec((1, frames, quantizers), audio_codes).map_err(|e| {
            MossTtsError::Inference {
                stage: "codec_decode_full",
                detail: e.to_string(),
            }
        })?;
        let lengths = Array1::from_vec(vec![frames as i64]);
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
        if audio_shape.len() != 3 || audio_shape[0] != 1 || audio_shape[1] != PLAYBACK_CHANNELS as i64 {
            return Err(MossTtsError::OutputFormat(format!(
                "expected codec audio shape [1, {}, samples], got {:?}",
                PLAYBACK_CHANNELS, audio_shape
            )));
        }
        let audio_len = outputs
            .get("audio_lengths")
            .and_then(|value| value.try_extract_tensor::<i64>().ok())
            .and_then(|(_, data)| data.first().copied())
            .map(|len| len.max(0) as usize)
            .unwrap_or(audio_shape[2] as usize)
            .min(audio_shape[2] as usize);
        let total_samples = audio_shape[2] as usize;
        let channels = PLAYBACK_CHANNELS as usize;
        let mut samples = Vec::with_capacity(audio_len * channels);
        for sample_index in 0..audio_len {
            for channel_index in 0..channels {
                samples.push(audio_data[channel_index * total_samples + sample_index]);
            }
        }
        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(samples),
            },
        })
    }
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

fn validate_session_io(
    session: &Session,
    model_name: &'static str,
    expected_inputs: &[String],
    expected_outputs: &[String],
) -> Result<(), MossTtsError> {
    for name in expected_inputs {
        if !session.inputs().iter().any(|input| input.name() == name) {
            return Err(MossTtsError::MetadataMismatch(format!(
                "{model_name} missing input '{name}'"
            )));
        }
    }
    for name in expected_outputs {
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
    outputs
        .get(name)
        .ok_or_else(|| MossTtsError::Inference {
            stage,
            detail: format!("missing output '{name}'"),
        })?
        .try_extract_tensor::<i64>()
        .map(|(_, data)| data.to_vec())
        .map_err(|e| MossTtsError::Inference {
            stage,
            detail: e.to_string(),
        })
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
}

#[derive(Debug, Clone, Deserialize)]
struct CodecMeta {
    format_version: u32,
    files: HashMap<String, String>,
    #[serde(default)]
    external_data_files: HashMap<String, Vec<String>>,
    codec_config: CodecConfig,
    onnx: CodecOnnxMeta,
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
    decode_input_names: Vec<String>,
    #[serde(default)]
    decode_output_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
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
            for file in ["decode_full.onnx", "codec_shared.data"] {
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
    "decode_output_names": ["global_hidden"]
  }
}"#,
            )
            .unwrap();
        }

        fn write_codec_meta(&self, sample_rate: u32, channels: u16, num_quantizers: u32) {
            std::fs::write(
                self.codec_dir.join("codec_browser_onnx_meta.json"),
                format!(
                    r#"{{
  "format_version": 2,
  "files": {{ "decode_full": "decode_full.onnx" }},
  "external_data_files": {{ "decode_full.onnx": ["codec_shared.data"] }},
  "codec_config": {{ "sample_rate": {sample_rate}, "channels": {channels}, "num_quantizers": {num_quantizers} }},
  "onnx": {{
    "decode_input_names": ["audio_codes", "audio_code_lengths"],
    "decode_output_names": ["audio", "audio_lengths"]
  }}
}}"#
                ),
            )
            .unwrap();
        }
    }
}
