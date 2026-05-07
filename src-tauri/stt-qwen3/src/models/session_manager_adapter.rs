use ndarray::{Array2, Array3, Array5};
use ort::value::Value;
use stt_core::{KvCache, Result, SessionManager, SttError};

use crate::decoder::decoder_init;
use crate::decoder::greedy_decode_logits;

use super::session::OnnxSessions;

pub struct SessionManagerAdapter {
    sessions: OnnxSessions,
}

impl SessionManagerAdapter {
    pub fn new(sessions: OnnxSessions) -> Self {
        Self { sessions }
    }

    pub fn into_inner(self) -> OnnxSessions {
        self.sessions
    }

    fn extract_kv_cache(keys_value: &Value, values_value: &Value) -> Result<KvCache> {
        let (keys_shape, keys_data) =
            keys_value
                .try_extract_tensor::<f32>()
                .map_err(|e| SttError::InferenceError {
                    model: "decoder".into(),
                    detail: format!("Failed to extract keys: {}", e),
                })?;

        let (values_shape, values_data) =
            values_value
                .try_extract_tensor::<f32>()
                .map_err(|e| SttError::InferenceError {
                    model: "decoder".into(),
                    detail: format!("Failed to extract values: {}", e),
                })?;

        let keys = Array5::from_shape_vec(
            (
                keys_shape[0] as usize,
                keys_shape[1] as usize,
                keys_shape[2] as usize,
                keys_shape[3] as usize,
                keys_shape[4] as usize,
            ),
            keys_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder".into(),
            detail: format!("Failed to reshape keys: {}", e),
        })?;

        let values = Array5::from_shape_vec(
            (
                values_shape[0] as usize,
                values_shape[1] as usize,
                values_shape[2] as usize,
                values_shape[3] as usize,
                values_shape[4] as usize,
            ),
            values_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder".into(),
            detail: format!("Failed to reshape values: {}", e),
        })?;

        Ok((keys, values))
    }
}

impl SessionManager for SessionManagerAdapter {
    fn run_encoder(&mut self, mel: &[Vec<f64>]) -> Result<Vec<Vec<f32>>> {
        crate::encoder::run_encoder(mel, &mut self.sessions)
    }

    fn run_decoder_init(
        &mut self,
        prompt_ids: &[u32],
        encoder_output: &[Vec<f32>],
    ) -> Result<(u32, KvCache)> {
        let (token, cache) = decoder_init(prompt_ids, encoder_output, &mut self.sessions)?;
        Ok((token, (cache.keys, cache.values)))
    }

    fn run_decoder_step(
        &mut self,
        input_embeds: &Array3<f32>,
        position_ids: &Array2<i64>,
        past_keys: &Array5<f32>,
        past_values: &Array5<f32>,
    ) -> Result<(u32, KvCache)> {
        let input_embeds_value =
            Value::from_array(input_embeds.clone()).map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create input_embeds value: {}", e),
            })?;

        let position_ids_value =
            Value::from_array(position_ids.clone()).map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create position_ids value: {}", e),
            })?;

        let past_keys_value =
            Value::from_array(past_keys.clone()).map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create past_keys value: {}", e),
            })?;

        let past_values_value =
            Value::from_array(past_values.clone()).map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create past_values value: {}", e),
            })?;

        let outputs = self
            .sessions
            .decoder_step
            .run(ort::inputs![
                "input_embeds" => input_embeds_value,
                "position_ids" => position_ids_value,
                "past_keys" => past_keys_value,
                "past_values" => past_values_value
            ])
            .map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Inference failed: {}", e),
            })?;

        let logits = &outputs[0];
        let (logits_shape, logits_data) =
            logits
                .try_extract_tensor::<f32>()
                .map_err(|e| SttError::InferenceError {
                    model: "decoder_step".into(),
                    detail: format!("Failed to extract logits: {}", e),
                })?;

        let next_token = greedy_decode_logits(logits_shape, logits_data, "decoder_step")?;

        let cache = Self::extract_kv_cache(&outputs[1], &outputs[2])?;

        Ok((next_token, cache))
    }
}
