pub mod audio;
pub mod decoder;
pub mod encoder;
pub mod models;
mod output;
pub mod prompt;
pub mod tokenizer;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use log::info;
use serde::{Deserialize, Serialize};
use stt_core::{
    AudioInput, Result as SttCoreResult, StreamingAudioChunk, StreamingStt, StreamingSttEvent,
    StreamingSttSession, StreamingTranscript, SttConfig, SttEngine, SttError, SttResult,
    TimingInfo,
};

use audio::loader;
use audio::mel::{compute_mel_spectrogram, create_mel_filterbank};
use audio::vad::{find_split_points, split_audio_at_points};
use decoder::{decoder_init, run_autoregressive_decode};
use encoder::run_encoder;
use models::session::{EmbeddingMatrix, OnnxSessions};
use output::parse_qwen3_output;
use prompt::build_prompt_ids_with_prefix;
use tokenizer::wrapper::TokenizerWrapper;

const TARGET_SAMPLE_RATE: u32 = 16000;
const DEFAULT_STREAM_CHUNK_SECONDS: f64 = 2.0;
const DEFAULT_STREAM_UNFIXED_CHUNK_NUM: usize = 2;
const DEFAULT_STREAM_UNFIXED_TOKEN_NUM: usize = 5;

const SUPPORTED_LANGUAGES: &[&str] = &[
    "zh", "en", "yue", "ja", "ko", "ar", "de", "fr", "es", "pt", "id", "it", "ru", "th", "vi",
    "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cz", "fil", "fa", "el", "ro", "hu", "mk",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Qwen3LoadTiming {
    pub total_ms: u64,
    pub onnx_sessions_ms: u64,
    pub embeddings_ms: u64,
    pub tokenizer_ms: u64,
    pub mel_filterbank_ms: u64,
}

pub struct Qwen3AsrEngine {
    model_dir: String,
    sessions: Mutex<OnnxSessions>,
    embeddings: EmbeddingMatrix,
    tokenizer: TokenizerWrapper,
    mel_filterbank: Vec<Vec<f64>>,
    load_timing: Qwen3LoadTiming,
}

impl Qwen3AsrEngine {
    pub fn new(model_dir: &str) -> Result<Self, SttError> {
        let total_start = Instant::now();
        let path = Path::new(model_dir);

        let onnx_start = Instant::now();
        let sessions = OnnxSessions::load(path)?;
        let onnx_sessions_ms = elapsed_ms(onnx_start);

        let embeddings_start = Instant::now();
        let embeddings = EmbeddingMatrix::load(path)?;
        let embeddings_ms = elapsed_ms(embeddings_start);

        let tokenizer_start = Instant::now();
        let tokenizer = TokenizerWrapper::load(path)?;
        let tokenizer_ms = elapsed_ms(tokenizer_start);

        let mel_start = Instant::now();
        let mel_filterbank = create_mel_filterbank();
        let mel_filterbank_ms = elapsed_ms(mel_start);

        let load_timing = Qwen3LoadTiming {
            total_ms: elapsed_ms(total_start),
            onnx_sessions_ms,
            embeddings_ms,
            tokenizer_ms,
            mel_filterbank_ms,
        };

        info!(
            "Qwen3 model loaded (total={}ms, onnx_sessions={}ms, embeddings={}ms, tokenizer={}ms, mel_filterbank={}ms)",
            load_timing.total_ms,
            load_timing.onnx_sessions_ms,
            load_timing.embeddings_ms,
            load_timing.tokenizer_ms,
            load_timing.mel_filterbank_ms
        );

        Ok(Self {
            model_dir: model_dir.to_string(),
            sessions: Mutex::new(sessions),
            embeddings,
            tokenizer,
            mel_filterbank,
            load_timing,
        })
    }

    pub fn load_timing(&self) -> Qwen3LoadTiming {
        self.load_timing
    }

    fn transcribe_samples(
        &self,
        samples: &[f32],
        config: &SttConfig,
    ) -> Result<SttResult, SttError> {
        let decoded = self.decode_samples_with_prefix(samples, config, "")?;

        Ok(stt_result_from_decoded_output(
            &decoded.text,
            config,
            decoded.audio_duration_sec,
            decoded.processing_time_sec,
            decoded.tokens_generated,
        ))
    }

    fn decode_samples_with_prefix(
        &self,
        samples: &[f32],
        config: &SttConfig,
        prefix: &str,
    ) -> Result<DecodedQwen3Output, SttError> {
        let start = Instant::now();
        let audio_duration = samples.len() as f64 / TARGET_SAMPLE_RATE as f64;

        let mel = compute_mel_spectrogram(samples, &self.mel_filterbank);

        let encoder_output = {
            let mut sessions = self.sessions.lock().unwrap();
            run_encoder(&mel, &mut sessions)?
        };

        let prompt_ids = build_prompt_ids_with_prefix(
            encoder_output.len(),
            config.language.as_deref(),
            prefix,
            &self.tokenizer,
        )?;

        let (init_token, cache) = {
            let mut sessions = self.sessions.lock().unwrap();
            decoder_init(&prompt_ids, &encoder_output, &mut sessions)?
        };

        let max_tokens = config.max_new_tokens.unwrap_or(512);
        let seq_len = prompt_ids.len();
        let generated_tokens = {
            let mut sessions = self.sessions.lock().unwrap();
            run_autoregressive_decode(
                init_token,
                cache,
                seq_len,
                max_tokens,
                &mut sessions,
                &self.embeddings,
            )?
        };

        let decoded_text = self.tokenizer.decode(&generated_tokens)?;

        let processing_time = start.elapsed().as_secs_f64();
        Ok(DecodedQwen3Output {
            text: decoded_text,
            audio_duration_sec: audio_duration,
            processing_time_sec: processing_time,
            tokens_generated: generated_tokens.len(),
        })
    }
}

struct DecodedQwen3Output {
    text: String,
    audio_duration_sec: f64,
    processing_time_sec: f64,
    tokens_generated: usize,
}

pub struct Qwen3StreamingSession<'a> {
    engine: &'a Qwen3AsrEngine,
    config: SttConfig,
    buffer: Vec<f32>,
    audio_accum: Vec<f32>,
    chunk_id: usize,
    raw_decoded: String,
    text: String,
    language: String,
    processing_time_sec: f64,
    tokens_generated: usize,
    chunk_size_samples: usize,
    unfixed_chunk_num: usize,
    unfixed_token_num: usize,
    events: VecDeque<StreamingSttEvent>,
    cancelled: bool,
    ended: bool,
}

impl<'a> Qwen3StreamingSession<'a> {
    fn new(engine: &'a Qwen3AsrEngine, config: SttConfig) -> Result<Self, SttError> {
        validate_language(&config)?;
        let chunk_seconds = config
            .stream_chunk_seconds
            .unwrap_or(DEFAULT_STREAM_CHUNK_SECONDS);
        if !chunk_seconds.is_finite() || chunk_seconds <= 0.0 {
            return Err(SttError::Other(format!(
                "stream_chunk_seconds must be greater than zero, got {chunk_seconds}"
            )));
        }

        let chunk_size_samples = (chunk_seconds * TARGET_SAMPLE_RATE as f64).round() as usize;
        if chunk_size_samples == 0 {
            return Err(SttError::Other(
                "stream_chunk_seconds produced an empty chunk size".into(),
            ));
        }

        Self {
            engine,
            config,
            buffer: Vec::new(),
            audio_accum: Vec::new(),
            chunk_id: 0,
            raw_decoded: String::new(),
            text: String::new(),
            language: "auto".to_string(),
            processing_time_sec: 0.0,
            tokens_generated: 0,
            chunk_size_samples,
            unfixed_chunk_num: DEFAULT_STREAM_UNFIXED_CHUNK_NUM,
            unfixed_token_num: DEFAULT_STREAM_UNFIXED_TOKEN_NUM,
            events: VecDeque::new(),
            cancelled: false,
            ended: false,
        }
        .with_configured_rollback()
    }

    fn with_configured_rollback(mut self) -> Result<Self, SttError> {
        self.unfixed_chunk_num = self
            .config
            .stream_unfixed_chunk_num
            .unwrap_or(DEFAULT_STREAM_UNFIXED_CHUNK_NUM);
        self.unfixed_token_num = self
            .config
            .stream_unfixed_token_num
            .unwrap_or(DEFAULT_STREAM_UNFIXED_TOKEN_NUM);
        Ok(self)
    }

    fn ensure_active(&self) -> Result<(), SttError> {
        if self.cancelled {
            Err(SttError::Other("streaming session cancelled".into()))
        } else {
            Ok(())
        }
    }

    fn append_audio(&mut self, chunk: StreamingAudioChunk) -> Result<(), SttError> {
        if chunk.sample_rate == 0 {
            return Err(SttError::AudioLoadError(
                "sample rate must be greater than zero".into(),
            ));
        }

        if chunk.sample_rate == TARGET_SAMPLE_RATE {
            self.buffer.extend_from_slice(&chunk.samples);
        } else {
            let resampled = loader::resample(&chunk.samples, chunk.sample_rate, TARGET_SAMPLE_RATE)
                .map_err(|e| SttError::AudioLoadError(format!("Resampling failed: {:?}", e)))?;
            self.buffer.extend_from_slice(&resampled);
        }
        Ok(())
    }

    fn process_ready_chunks(&mut self) -> Result<(), SttError> {
        while self.buffer.len() >= self.chunk_size_samples {
            let chunk: Vec<f32> = self.buffer.drain(..self.chunk_size_samples).collect();
            self.audio_accum.extend_from_slice(&chunk);
            self.process_accumulated_audio(true)?;
        }
        Ok(())
    }

    fn process_tail(&mut self) -> Result<(), SttError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        self.audio_accum.append(&mut self.buffer);
        self.process_accumulated_audio(false)
    }

    fn process_accumulated_audio(&mut self, emit_partial: bool) -> Result<(), SttError> {
        let prefix = self.rollback_prefix()?;
        let decoded =
            self.engine
                .decode_samples_with_prefix(&self.audio_accum, &self.config, &prefix)?;

        self.raw_decoded = format!("{prefix}{}", decoded.text);
        let parsed = parse_qwen3_output(&self.raw_decoded, self.config.language.as_deref());
        self.text = parsed.text;
        self.language = parsed.language;
        self.processing_time_sec += decoded.processing_time_sec;
        self.tokens_generated += decoded.tokens_generated;

        if emit_partial {
            self.events
                .push_back(StreamingSttEvent::Partial(self.current_transcript()));
        }
        self.chunk_id += 1;
        Ok(())
    }

    fn rollback_prefix(&self) -> Result<String, SttError> {
        rollback_prefix(
            &self.raw_decoded,
            self.chunk_id,
            self.unfixed_chunk_num,
            self.unfixed_token_num,
            &self.engine.tokenizer,
        )
    }

    fn current_transcript(&self) -> StreamingTranscript {
        StreamingTranscript {
            text: self.text.clone(),
            language: Some(self.language.clone()),
            start_time_sec: Some(0.0),
            end_time_sec: Some(self.audio_accum.len() as f64 / TARGET_SAMPLE_RATE as f64),
            confidence: None,
        }
    }

    fn current_result(&self) -> SttResult {
        let audio_duration = self.audio_accum.len() as f64 / TARGET_SAMPLE_RATE as f64;
        let rtf = if audio_duration > 0.0 {
            self.processing_time_sec / audio_duration
        } else {
            0.0
        };

        SttResult {
            text: self.text.clone(),
            language: self.language.clone(),
            confidence: None,
            timing: TimingInfo {
                audio_duration_sec: audio_duration,
                processing_time_sec: self.processing_time_sec,
                rtf,
                tokens_generated: Some(self.tokens_generated),
            },
        }
    }
}

fn validate_language(config: &SttConfig) -> Result<(), SttError> {
    if let Some(ref lang) = config.language {
        if !SUPPORTED_LANGUAGES.contains(&lang.as_str()) {
            return Err(SttError::UnsupportedLanguage(lang.clone()));
        }
    }
    Ok(())
}

fn rollback_prefix(
    raw_decoded: &str,
    chunk_id: usize,
    unfixed_chunk_num: usize,
    unfixed_token_num: usize,
    tokenizer: &TokenizerWrapper,
) -> Result<String, SttError> {
    if chunk_id < unfixed_chunk_num || raw_decoded.is_empty() {
        return Ok(String::new());
    }

    let tokens = tokenizer.encode(raw_decoded)?;
    rollback_prefix_from_tokens(&tokens, unfixed_token_num, tokenizer)
}

fn rollback_prefix_from_tokens(
    tokens: &[u32],
    unfixed_token_num: usize,
    tokenizer: &TokenizerWrapper,
) -> Result<String, SttError> {
    rollback_prefix_from_tokens_with_decoder(tokens, unfixed_token_num, |ids| tokenizer.decode(ids))
}

fn rollback_prefix_from_tokens_with_decoder(
    tokens: &[u32],
    unfixed_token_num: usize,
    decode: impl Fn(&[u32]) -> Result<String, SttError>,
) -> Result<String, SttError> {
    let mut drop_count = unfixed_token_num.min(tokens.len());
    loop {
        let keep_len = tokens.len().saturating_sub(drop_count);
        let prefix = decode(&tokens[..keep_len])?;
        if !prefix.contains('\u{fffd}') || keep_len == 0 {
            return Ok(prefix);
        }
        drop_count += 1;
    }
}

#[async_trait]
impl StreamingStt for Qwen3AsrEngine {
    async fn start_stream(
        &self,
        config: SttConfig,
    ) -> SttCoreResult<Box<dyn StreamingSttSession + Send + '_>> {
        Ok(Box::new(Qwen3StreamingSession::new(self, config)?))
    }
}

#[async_trait]
impl StreamingSttSession for Qwen3StreamingSession<'_> {
    async fn push_audio(&mut self, chunk: StreamingAudioChunk) -> SttCoreResult<()> {
        self.ensure_active()?;
        if self.ended {
            return Err(SttError::Other(
                "cannot push audio after stream finished".into(),
            ));
        }

        self.append_audio(chunk)?;
        self.process_ready_chunks()
    }

    async fn next_event(&mut self) -> SttCoreResult<Option<StreamingSttEvent>> {
        self.ensure_active()?;
        Ok(self.events.pop_front())
    }

    async fn finish(&mut self) -> SttCoreResult<SttResult> {
        self.ensure_active()?;
        if !self.ended {
            self.process_tail()?;
            let result = self.current_result();
            self.events
                .push_back(StreamingSttEvent::End(result.clone()));
            self.ended = true;
            Ok(result)
        } else {
            Ok(self.current_result())
        }
    }

    async fn cancel(&mut self) -> SttCoreResult<()> {
        self.buffer.clear();
        self.audio_accum.clear();
        self.events.clear();
        self.cancelled = true;
        Ok(())
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn stt_result_from_decoded_output(
    decoded_text: &str,
    config: &SttConfig,
    audio_duration: f64,
    processing_time: f64,
    tokens_generated: usize,
) -> SttResult {
    let parsed = parse_qwen3_output(decoded_text, config.language.as_deref());
    let rtf = if audio_duration > 0.0 {
        processing_time / audio_duration
    } else {
        0.0
    };

    SttResult {
        text: parsed.text,
        language: parsed.language,
        confidence: None,
        timing: TimingInfo {
            audio_duration_sec: audio_duration,
            processing_time_sec: processing_time,
            rtf,
            tokens_generated: Some(tokens_generated),
        },
    }
}

#[async_trait]
impl SttEngine for Qwen3AsrEngine {
    fn engine_name(&self) -> &str {
        "qwen3-asr-0.6b"
    }

    fn supported_languages(&self) -> &[&str] {
        SUPPORTED_LANGUAGES
    }

    async fn transcribe(
        &self,
        input: AudioInput,
        config: SttConfig,
    ) -> Result<SttResult, SttError> {
        validate_language(&config)?;

        let samples = match input {
            AudioInput::FilePath(path) => loader::load_audio_from_file(&path)?,
            AudioInput::Bytes(data) => loader::load_audio_from_bytes(&data)?,
            AudioInput::Samples(data, rate) => {
                let mut s = data;
                if rate != 16000 {
                    s = loader::resample(&s, rate, 16000).map_err(|e| {
                        SttError::AudioLoadError(format!("Resampling failed: {:?}", e))
                    })?;
                }
                loader::validate_samples(&s, 16000)?;
                s
            }
        };

        let duration = samples.len() as f64 / 16000.0;
        let chunk_seconds = config.chunk_seconds.unwrap_or(30.0);

        if config.enable_vad && duration >= 45.0 {
            let split_points = find_split_points(&samples, chunk_seconds);
            let chunks = split_audio_at_points(&samples, &split_points);

            let mut full_text = String::new();
            let mut language = config
                .language
                .clone()
                .unwrap_or_else(|| "auto".to_string());
            let mut total_processing = 0.0;
            let mut total_tokens = 0;

            for chunk in chunks {
                let result = self.transcribe_samples(chunk, &config)?;
                if !result.text.is_empty() {
                    if !full_text.is_empty() {
                        full_text.push(' ');
                    }
                    full_text.push_str(&result.text);
                }
                if config.language.is_none() && language == "auto" && result.language != "auto" {
                    language = result.language;
                }
                total_processing += result.timing.processing_time_sec;
                total_tokens += result.timing.tokens_generated.unwrap_or(0);
            }

            Ok(SttResult {
                text: full_text,
                language,
                confidence: None,
                timing: TimingInfo {
                    audio_duration_sec: duration,
                    processing_time_sec: total_processing,
                    rtf: total_processing / duration,
                    tokens_generated: Some(total_tokens),
                },
            })
        } else {
            self.transcribe_samples(&samples, &config)
        }
    }

    async fn health_check(&self) -> Result<bool, SttError> {
        let model_dir = Path::new(&self.model_dir);
        let onnx_dir = model_dir.join("onnx_models");

        let has_any = |base_dir: &Path, candidates: &[&str]| {
            candidates
                .iter()
                .any(|candidate| base_dir.join(candidate).exists())
        };

        if !has_any(&onnx_dir, &["encoder.int4.onnx", "encoder.onnx"]) {
            return Err(SttError::InferenceError {
                model: "encoder".into(),
                detail: format!(
                    "Missing file: one of {:?} in {}",
                    ["encoder.int4.onnx", "encoder.onnx"],
                    onnx_dir.display()
                ),
            });
        }

        if !has_any(&onnx_dir, &["decoder_init.int4.onnx", "decoder_init.onnx"]) {
            return Err(SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!(
                    "Missing file: one of {:?} in {}",
                    ["decoder_init.int4.onnx", "decoder_init.onnx"],
                    onnx_dir.display()
                ),
            });
        }

        if !has_any(&onnx_dir, &["decoder_step.int4.onnx", "decoder_step.onnx"]) {
            return Err(SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!(
                    "Missing file: one of {:?} in {}",
                    ["decoder_step.int4.onnx", "decoder_step.onnx"],
                    onnx_dir.display()
                ),
            });
        }

        for (file, model_name) in [
            ("embed_tokens.bin", "embed_tokens"),
            ("tokenizer.json", "tokenizer.json"),
            ("config.json", "config.json"),
        ] {
            let path = model_dir.join(file);
            if !path.exists() {
                return Err(SttError::InferenceError {
                    model: model_name.into(),
                    detail: format!("Missing file: {}", path.display()),
                });
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stt_result_uses_parsed_auto_output() {
        let result = stt_result_from_decoded_output(
            "language English<asr_text>  parsed text  ",
            &SttConfig::default(),
            2.0,
            1.0,
            10,
        );

        assert_eq!(result.text, "parsed text");
        assert_eq!(result.language, "en");
        assert_eq!(result.timing.tokens_generated, Some(10));
    }

    #[test]
    fn stt_result_preserves_forced_language_output() {
        let config = SttConfig {
            language: Some("zh".to_string()),
            ..Default::default()
        };

        let result = stt_result_from_decoded_output(
            "language English<asr_text>  parsed text  ",
            &config,
            2.0,
            1.0,
            10,
        );

        assert_eq!(result.text, "language English<asr_text>  parsed text");
        assert_eq!(result.language, "zh");
    }

    #[test]
    fn elapsed_ms_saturates_to_u64() {
        let elapsed = elapsed_ms(Instant::now());

        assert!(elapsed < 1_000);
    }

    #[test]
    fn rollback_prefix_keeps_stable_tokens() {
        let tokens = vec![1, 2, 3, 4, 5];

        let prefix = rollback_prefix_from_tokens_with_decoder(&tokens, 2, |ids| {
            Ok(ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
        })
        .unwrap();

        assert_eq!(prefix, "1,2,3");
    }

    #[test]
    fn rollback_prefix_drops_all_tokens_when_unfixed_count_is_large() {
        let tokens = vec![1, 2, 3];

        let prefix = rollback_prefix_from_tokens_with_decoder(&tokens, 99, |ids| {
            Ok(ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
        })
        .unwrap();

        assert_eq!(prefix, "");
    }

    #[test]
    fn rollback_prefix_drops_more_tokens_for_replacement_char() {
        let tokens = vec![1, 2, 3, 4, 5];

        let prefix = rollback_prefix_from_tokens_with_decoder(&tokens, 1, |ids| {
            if ids.len() > 2 {
                Ok("bad\u{fffd}".to_string())
            } else {
                Ok("stable".to_string())
            }
        })
        .unwrap();

        assert_eq!(prefix, "stable");
    }
}
