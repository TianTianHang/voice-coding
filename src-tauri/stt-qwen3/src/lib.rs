pub mod audio;
pub mod decoder;
pub mod encoder;
pub mod models;
pub mod prompt;
pub mod tokenizer;

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use stt_core::{AudioInput, SttConfig, SttEngine, SttError, SttResult, TimingInfo};

use audio::loader;
use audio::mel::{compute_mel_spectrogram, create_mel_filterbank};
use audio::vad::{find_split_points, split_audio_at_points};
use decoder::{decoder_init, embed_and_fuse, run_autoregressive_decode};
use encoder::run_encoder;
use models::session::{EmbeddingMatrix, OnnxSessions};
use prompt::build_prompt_ids;
use tokenizer::wrapper::TokenizerWrapper;

const SUPPORTED_LANGUAGES: &[&str] = &[
    "zh", "en", "yue", "ja", "ko", "ar", "de", "fr", "es", "pt", "id", "it", "ru", "th", "vi",
    "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cz", "fil", "fa", "el", "ro", "hu", "mk",
];

pub struct Qwen3AsrEngine {
    model_dir: String,
    sessions: Mutex<OnnxSessions>,
    embeddings: EmbeddingMatrix,
    tokenizer: TokenizerWrapper,
    mel_filterbank: Vec<Vec<f64>>,
}

impl Qwen3AsrEngine {
    pub fn new(model_dir: &str) -> Result<Self, SttError> {
        let path = Path::new(model_dir);
        let sessions = OnnxSessions::load(path)?;
        let embeddings = EmbeddingMatrix::load(path)?;
        let tokenizer = TokenizerWrapper::load(path)?;
        let mel_filterbank = create_mel_filterbank();

        Ok(Self {
            model_dir: model_dir.to_string(),
            sessions: Mutex::new(sessions),
            embeddings,
            tokenizer,
            mel_filterbank,
        })
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

        let n_audio_tokens = encoder_output.len();

        let prompt_ids =
            build_prompt_ids(n_audio_tokens, config.language.as_deref(), &self.tokenizer)?;

        let input_embeds = embed_and_fuse(&prompt_ids, &encoder_output, &self.embeddings)?;

        let (init_token, cache) = {
            let mut sessions = self.sessions.lock().unwrap();
            decoder_init(&input_embeds, &mut sessions)?
        };

        let max_tokens = config.max_new_tokens.unwrap_or(512);
        let seq_len = input_embeds.shape()[1];
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

        let text = self.tokenizer.decode(&generated_tokens)?;

        let processing_time = start.elapsed().as_secs_f64();
        let rtf = processing_time / audio_duration;

        Ok(SttResult {
            text,
            language: config
                .language
                .clone()
                .unwrap_or_else(|| "auto".to_string()),
            confidence: None,
            timing: TimingInfo {
                audio_duration_sec: audio_duration,
                processing_time_sec: processing_time,
                rtf,
                tokens_generated: Some(generated_tokens.len()),
            },
        })
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
            let mut total_processing = 0.0;
            let mut total_tokens = 0;

            for chunk in chunks {
                let result = self.transcribe_samples(chunk, &config)?;
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&result.text);
                total_processing += result.timing.processing_time_sec;
                total_tokens += result.timing.tokens_generated.unwrap_or(0);
            }

            Ok(SttResult {
                text: full_text,
                language: config
                    .language
                    .clone()
                    .unwrap_or_else(|| "auto".to_string()),
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

        let required_files = [
            onnx_dir.join("encoder_conv.onnx"),
            onnx_dir.join("encoder_transformer.onnx"),
            model_dir.join("embed_tokens.bin"),
            model_dir.join("tokenizer.json"),
        ];

        for file in &required_files {
            if !file.exists() {
                return Err(SttError::InferenceError {
                    model: "health_check".into(),
                    detail: format!("Missing file: {}", file.display()),
                });
            }
        }

        Ok(true)
    }
}
