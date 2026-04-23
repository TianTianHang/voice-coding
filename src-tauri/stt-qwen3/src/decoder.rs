use ndarray::Array3;
use ort::value::Value;

use crate::models::session::EmbeddingMatrix;
use crate::prompt::AUDIO_PAD_ID;
use stt_core::SttError;

pub fn embed_and_fuse(
    token_ids: &[u32],
    encoder_output: &[Vec<f32>],
    embeddings: &EmbeddingMatrix,
) -> Result<Array3<f32>, SttError> {
    let seq_len = token_ids.len();
    let hidden_size = embeddings.hidden_size;

    let mut input_embeds = Array3::zeros((1, seq_len, hidden_size));

    let audio_pad_positions: Vec<usize> = token_ids
        .iter()
        .enumerate()
        .filter(|(_, &id)| id == AUDIO_PAD_ID)
        .map(|(i, _)| i)
        .collect();

    if audio_pad_positions.len() != encoder_output.len() {
        return Err(SttError::InferenceError {
            model: "embedding_fusion".into(),
            detail: format!(
                "Encoder output count ({}) does not match audio pad token count ({})",
                encoder_output.len(),
                audio_pad_positions.len()
            ),
        });
    }

    for (pos, &token_id) in token_ids.iter().enumerate() {
        if token_id == AUDIO_PAD_ID {
            let pad_idx = audio_pad_positions.iter().position(|&p| p == pos).unwrap();
            for (d, &v) in encoder_output[pad_idx].iter().enumerate() {
                input_embeds[[0, pos, d]] = v;
            }
        } else {
            let emb = embeddings.get_embedding(token_id);
            for (d, &v) in emb.iter().enumerate() {
                input_embeds[[0, pos, d]] = v;
            }
        }
    }

    Ok(input_embeds)
}

pub struct KvCache {
    pub keys: ndarray::Array4<f32>,
    pub values: ndarray::Array4<f32>,
}

pub fn decoder_init(
    input_embeds: &Array3<f32>,
    sessions: &mut crate::models::session::OnnxSessions,
) -> Result<(u32, KvCache), SttError> {
    let seq_len = input_embeds.shape()[1];
    let position_ids_data: Vec<f32> = (0..seq_len).map(|i| i as f32).collect();
    let position_ids =
        ndarray::Array2::from_shape_vec((1, seq_len), position_ids_data).map_err(|e| {
            SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Failed to create position_ids: {}", e),
            }
        })?;

    let input_embeds_value =
        Value::from_array(input_embeds.clone()).map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create input_embeds value: {}", e),
        })?;

    let position_ids_value =
        Value::from_array(position_ids.clone()).map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create position_ids value: {}", e),
        })?;

    let outputs = sessions
        .decoder_init
        .run(vec![
            ("input_embeds", input_embeds_value),
            ("position_ids", position_ids_value),
        ])
        .map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Inference failed: {}", e),
        })?;

    let logits = &outputs[0];
    let (logits_shape, logits_data) =
        logits
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Failed to extract logits: {}", e),
            })?;

    let logits_vec = logits_data.to_vec();
    let logits_array = ndarray::Array3::from_shape_vec(
        (
            logits_shape[0] as usize,
            logits_shape[1] as usize,
            logits_shape[2] as usize,
        ),
        logits_vec,
    )
    .map_err(|e| SttError::InferenceError {
        model: "decoder_init".into(),
        detail: format!("Failed to reshape logits: {}", e),
    })?;

    let first_token = greedy_decode(&logits_array.view());

    let present_keys = &outputs[1];
    let (keys_shape, keys_data) =
        present_keys
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Failed to extract present_keys: {}", e),
            })?;

    let present_values = &outputs[2];
    let (values_shape, values_data) =
        present_values
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Failed to extract present_values: {}", e),
            })?;

    let cache = KvCache {
        keys: ndarray::Array4::from_shape_vec(
            (
                keys_shape[0] as usize,
                keys_shape[1] as usize,
                keys_shape[2] as usize,
                keys_shape[3] as usize,
            ),
            keys_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to reshape keys: {}", e),
        })?,
        values: ndarray::Array4::from_shape_vec(
            (
                values_shape[0] as usize,
                values_shape[1] as usize,
                values_shape[2] as usize,
                values_shape[3] as usize,
            ),
            values_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to reshape values: {}", e),
        })?,
    };

    Ok((first_token, cache))
}

pub fn decoder_step(
    token_id: u32,
    position: usize,
    cache: &KvCache,
    embeddings: &EmbeddingMatrix,
    sessions: &mut crate::models::session::OnnxSessions,
) -> Result<(u32, KvCache), SttError> {
    let emb = embeddings.get_embedding(token_id);
    let input_embeds =
        ndarray::Array3::from_shape_vec((1, 1, emb.len()), emb.to_vec()).map_err(|e| {
            SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create input embeds: {}", e),
            }
        })?;

    let position_ids =
        ndarray::Array2::from_shape_vec((1, 1), vec![position as f32]).map_err(|e| {
            SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to create position_ids: {}", e),
            }
        })?;

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
        Value::from_array(cache.keys.clone()).map_err(|e| SttError::InferenceError {
            model: "decoder_step".into(),
            detail: format!("Failed to create past_keys value: {}", e),
        })?;
    let past_values_value =
        Value::from_array(cache.values.clone()).map_err(|e| SttError::InferenceError {
            model: "decoder_step".into(),
            detail: format!("Failed to create past_values value: {}", e),
        })?;

    let outputs = sessions
        .decoder_step
        .run(vec![
            ("input_embeds", input_embeds_value),
            ("position_ids", position_ids_value),
            ("past_keys", past_keys_value),
            ("past_values", past_values_value),
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

    let logits_vec = logits_data.to_vec();
    let logits_array = ndarray::Array3::from_shape_vec(
        (
            logits_shape[0] as usize,
            logits_shape[1] as usize,
            logits_shape[2] as usize,
        ),
        logits_vec,
    )
    .map_err(|e| SttError::InferenceError {
        model: "decoder_step".into(),
        detail: format!("Failed to reshape logits: {}", e),
    })?;

    let next_token = greedy_decode(&logits_array.view());

    let present_keys = &outputs[1];
    let (keys_shape, keys_data) =
        present_keys
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to extract present_keys: {}", e),
            })?;

    let present_values = &outputs[2];
    let (values_shape, values_data) =
        present_values
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "decoder_step".into(),
                detail: format!("Failed to extract present_values: {}", e),
            })?;

    let new_cache = KvCache {
        keys: ndarray::Array4::from_shape_vec(
            (
                keys_shape[0] as usize,
                keys_shape[1] as usize,
                keys_shape[2] as usize,
                keys_shape[3] as usize,
            ),
            keys_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder_step".into(),
            detail: format!("Failed to reshape keys: {}", e),
        })?,
        values: ndarray::Array4::from_shape_vec(
            (
                values_shape[0] as usize,
                values_shape[1] as usize,
                values_shape[2] as usize,
                values_shape[3] as usize,
            ),
            values_data.to_vec(),
        )
        .map_err(|e| SttError::InferenceError {
            model: "decoder_step".into(),
            detail: format!("Failed to reshape values: {}", e),
        })?,
    };

    Ok((next_token, new_cache))
}

fn greedy_decode(logits: &ndarray::ArrayView3<f32>) -> u32 {
    let last_logits = logits.slice(ndarray::s![0, -1, ..]);
    let mut max_idx = 0usize;
    let mut max_val = f32::NEG_INFINITY;
    for (i, &v) in last_logits.iter().enumerate() {
        if v > max_val {
            max_val = v;
            max_idx = i;
        }
    }
    max_idx as u32
}

pub fn run_autoregressive_decode(
    init_token: u32,
    init_cache: KvCache,
    max_new_tokens: usize,
    sessions: &mut crate::models::session::OnnxSessions,
    embeddings: &EmbeddingMatrix,
) -> Result<Vec<u32>, SttError> {
    use crate::prompt::{ENDOFTEXT_ID, IM_END_ID};

    let mut tokens = vec![init_token];
    let mut cache = init_cache;

    for position in 0..max_new_tokens {
        let (next_token, new_cache) = decoder_step(
            *tokens.last().unwrap(),
            position,
            &cache,
            embeddings,
            sessions,
        )?;

        if next_token == IM_END_ID || next_token == ENDOFTEXT_ID {
            break;
        }

        tokens.push(next_token);
        cache = new_cache;
    }

    Ok(tokens)
}
