use std::path::Path;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

use stt_core::SttError;

pub struct OnnxSessions {
    pub encoder: Session,
    pub decoder_init: Session,
    pub decoder_step: Session,
}

impl OnnxSessions {
    pub fn load(model_dir: &Path) -> Result<Self, SttError> {
        let onnx_dir = model_dir.join("onnx_models");

        let encoder_path =
            Self::prefer_existing(&onnx_dir, &["encoder.int4.onnx", "encoder.onnx"], "encoder")?;
        let encoder = Self::create_session(&encoder_path, "encoder")?;

        let decoder_init_path = Self::prefer_existing(
            &onnx_dir,
            &["decoder_init.int4.onnx", "decoder_init.onnx"],
            "decoder_init",
        )?;
        let decoder_init = Self::create_session(&decoder_init_path, "decoder_init")?;

        let decoder_step_path = Self::prefer_existing(
            &onnx_dir,
            &["decoder_step.int4.onnx", "decoder_step.onnx"],
            "decoder_step",
        )?;
        let decoder_step = Self::create_session(&decoder_step_path, "decoder_step")?;

        Ok(Self {
            encoder,
            decoder_init,
            decoder_step,
        })
    }

    fn prefer_existing(
        onnx_dir: &Path,
        candidates: &[&str],
        model_name: &str,
    ) -> Result<std::path::PathBuf, SttError> {
        for candidate in candidates {
            let path = onnx_dir.join(candidate);
            if path.exists() {
                return Ok(path);
            }
        }

        Err(SttError::InferenceError {
            model: model_name.into(),
            detail: format!("None of the expected model files exist: {:?}", candidates),
        })
    }

    fn create_session(model_path: &Path, model_name: &str) -> Result<Session, SttError> {
        if !model_path.exists() {
            return Err(SttError::InferenceError {
                model: model_name.into(),
                detail: format!("Model file not found: {}", model_path.display()),
            });
        }

        Session::builder()
            .and_then(|b| {
                b.with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(0)?
                    .commit_from_file(model_path)
            })
            .map_err(|e| SttError::InferenceError {
                model: model_name.into(),
                detail: format!("Failed to create session: {}", e),
            })
    }
}

pub struct EmbeddingMatrix {
    pub data: Vec<f32>,
    pub vocab_size: usize,
    pub hidden_size: usize,
}

impl EmbeddingMatrix {
    pub const EXPECTED_VOCAB_SIZE: usize = 151936;
    pub const EXPECTED_HIDDEN_SIZE: usize = 1024;

    pub fn load(model_dir: &Path) -> Result<Self, SttError> {
        let path = model_dir.join("embed_tokens.bin");
        if !path.exists() {
            return Err(SttError::InferenceError {
                model: "embed_tokens".into(),
                detail: format!("Embedding file not found: {}", path.display()),
            });
        }

        let data = std::fs::read(&path).map_err(|e| SttError::InferenceError {
            model: "embed_tokens".into(),
            detail: format!("Failed to read embedding file: {}", e),
        })?;

        let fp16_bytes = Self::EXPECTED_VOCAB_SIZE * Self::EXPECTED_HIDDEN_SIZE * 2;
        let fp32_bytes = Self::EXPECTED_VOCAB_SIZE * Self::EXPECTED_HIDDEN_SIZE * 4;

        let floats: Vec<f32> = if data.len() == fp16_bytes {
            data.chunks_exact(2)
                .map(|chunk| f16_le_bytes_to_f32([chunk[0], chunk[1]]))
                .collect()
        } else if data.len() == fp32_bytes {
            data.chunks_exact(4)
                .map(|chunk| {
                    let bytes: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                    f32::from_le_bytes(bytes)
                })
                .collect()
        } else {
            return Err(SttError::InferenceError {
                model: "embed_tokens".into(),
                detail: format!(
                    "Embedding file size mismatch: expected {} bytes (fp16) or {} bytes (fp32), got {} bytes",
                    fp16_bytes,
                    fp32_bytes,
                    data.len()
                ),
            });
        };

        assert_eq!(
            floats.len(),
            Self::EXPECTED_VOCAB_SIZE * Self::EXPECTED_HIDDEN_SIZE
        );

        Ok(Self {
            data: floats,
            vocab_size: Self::EXPECTED_VOCAB_SIZE,
            hidden_size: Self::EXPECTED_HIDDEN_SIZE,
        })
    }

    pub fn get_embedding(&self, token_id: u32) -> Result<&[f32], SttError> {
        if token_id as usize >= self.vocab_size {
            return Err(SttError::InferenceError {
                model: "embed_tokens".into(),
                detail: format!(
                    "token id {} is outside embedding vocabulary size {}",
                    token_id, self.vocab_size
                ),
            });
        }

        let start = token_id as usize * self.hidden_size;
        let end = start + self.hidden_size;
        self.data
            .get(start..end)
            .ok_or_else(|| SttError::InferenceError {
                model: "embed_tokens".into(),
                detail: format!(
                    "embedding slice for token id {} is outside matrix bounds",
                    token_id
                ),
            })
    }
}

fn f16_le_bytes_to_f32(bytes: [u8; 2]) -> f32 {
    let half = u16::from_le_bytes(bytes);
    let sign = ((half >> 15) & 0x1) as u32;
    let exp = ((half >> 10) & 0x1f) as u32;
    let frac = (half & 0x03ff) as u32;

    let bits = match exp {
        0 => {
            if frac == 0 {
                sign << 31
            } else {
                let mut frac = frac;
                let mut exp = -14i32;
                while (frac & 0x0400) == 0 {
                    frac <<= 1;
                    exp -= 1;
                }
                frac &= 0x03ff;
                let exp_bits = ((exp + 127) as u32) << 23;
                let frac_bits = frac << 13;
                (sign << 31) | exp_bits | frac_bits
            }
        }
        0x1f => (sign << 31) | 0x7f80_0000 | (frac << 13),
        _ => {
            let exp_bits = ((exp as i32 - 15 + 127) as u32) << 23;
            let frac_bits = frac << 13;
            (sign << 31) | exp_bits | frac_bits
        }
    };

    f32::from_bits(bits)
}
