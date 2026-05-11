pub mod audio;
pub mod decoder;
pub mod encoder;
pub mod models;
mod output;
pub mod prompt;
pub mod tokenizer;

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use log::info;
use serde::{Deserialize, Serialize};
use stt_core::{AudioInput, SttConfig, SttEngine, SttError, SttResult, TimingInfo};

use audio::loader;
use audio::mel::{compute_mel_spectrogram, create_mel_filterbank};
use audio::vad::{find_split_points, split_audio_at_points};
use decoder::{decoder_init, run_autoregressive_decode};
use encoder::run_encoder;
use models::session::{EmbeddingMatrix, OnnxSessions};
use output::parse_qwen3_output;
use prompt::build_prompt_ids;
use tokenizer::wrapper::TokenizerWrapper;

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
        let start = Instant::now();
        let audio_duration = samples.len() as f64 / 16000.0;

        let mel = compute_mel_spectrogram(samples, &self.mel_filterbank);

        let encoder_output = {
            let mut sessions = self.sessions.lock().unwrap();
            run_encoder(&mel, &mut sessions)?
        };

        let prompt_ids = build_prompt_ids(encoder_output.len(), config.language.as_deref(), &self.tokenizer)?;

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
        Ok(stt_result_from_decoded_output(
            &decoded_text,
            config,
            audio_duration,
            processing_time,
            generated_tokens.len(),
        ))
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
        if let Some(ref lang) = config.language {
            if !SUPPORTED_LANGUAGES.contains(&lang.as_str()) {
                return Err(SttError::UnsupportedLanguage(lang.clone()));
            }
        }

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
            candidates.iter().any(|candidate| base_dir.join(candidate).exists())
        };

        if !has_any(&onnx_dir, &["encoder.int4.onnx", "encoder.onnx"]) {
            return Err(SttError::InferenceError {
                model: "encoder".into(),
                detail: format!("Missing file: one of {:?} in {}", ["encoder.int4.onnx", "encoder.onnx"], onnx_dir.display()),
            });
        }

        if !has_any(&onnx_dir, &["decoder_init.int4.onnx", "decoder_init.onnx"]) {
            return Err(SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Missing file: one of {:?} in {}", ["decoder_init.int4.onnx", "decoder_init.onnx"], onnx_dir.display()),
            });
        }

        if !has_any(&onnx_dir, &["decoder_step.int4.onnx", "decoder_step.onnx"]) {
            return Err(SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Missing file: one of {:?} in {}", ["decoder_step.int4.onnx", "decoder_step.onnx"], onnx_dir.display()),
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
}
