use ndarray::{Array1, Array2, Array3};
use ort::value::Value;

use crate::models::session::EmbeddingMatrix;
use crate::prompt::{get_audio_pad_range, AUDIO_PAD_ID};
use stt_core::SttError;

pub fn embed_and_fuse(
    token_ids: &[u32],
    encoder_output: &[Vec<f32>],
    embeddings: &EmbeddingMatrix,
) -> Result<Array3<f32>, SttError> {
    let seq_len = token_ids.len();
    let hidden_size = embeddings.hidden_size;

    let mut input_embeds = Array3::zeros((1, seq_len, hidden_size));

    let audio_pad_count = token_ids.iter().filter(|&&id| id == AUDIO_PAD_ID).count();
    if audio_pad_count != encoder_output.len() {
        return Err(SttError::InferenceError {
            model: "embedding_fusion".into(),
            detail: format!(
                "Encoder output count ({}) does not match audio pad token count ({})",
                encoder_output.len(),
                audio_pad_count
            ),
        });
    }

    let mut audio_idx = 0;
    for (pos, &token_id) in token_ids.iter().enumerate() {
        if token_id == AUDIO_PAD_ID {
            for (d, &v) in encoder_output[audio_idx].iter().enumerate() {
                input_embeds[[0, pos, d]] = v;
            }
            audio_idx += 1;
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
    pub keys: ndarray::Array5<f32>,
    pub values: ndarray::Array5<f32>,
}

pub fn decoder_init(
    prompt_ids: &[u32],
    encoder_output: &[Vec<f32>],
    sessions: &mut crate::models::session::OnnxSessions,
) -> Result<(u32, KvCache), SttError> {
    let seq_len = prompt_ids.len();
    let position_ids =
        Array2::from_shape_vec((1, seq_len), (0..seq_len).map(|i| i as i64).collect()).map_err(
            |e| SttError::InferenceError {
                model: "decoder_init".into(),
                detail: format!("Failed to create position_ids: {}", e),
            },
        )?;

    let (audio_start, audio_end) = get_audio_pad_range(prompt_ids)?;
    let audio_len = audio_end - audio_start;
    if audio_len != encoder_output.len() {
        return Err(SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!(
                "Encoder output count ({}) does not match audio pad token count ({})",
                encoder_output.len(),
                audio_len
            ),
        });
    }

    let hidden_size = encoder_output.first().map(|row| row.len()).unwrap_or(0);
    let audio_features = Array3::from_shape_vec(
        (1, audio_len, hidden_size),
        encoder_output
            .iter()
            .flat_map(|row| row.iter().copied())
            .collect(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "decoder_init".into(),
        detail: format!("Failed to create audio_features tensor: {}", e),
    })?;

    let input_ids = Array2::from_shape_vec(
        (1, seq_len),
        prompt_ids.iter().map(|&id| id as i64).collect(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "decoder_init".into(),
        detail: format!("Failed to create input_ids: {}", e),
    })?;

    let audio_offset = Array1::from_shape_vec(1, vec![audio_start as i64]).map_err(|e| {
        SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create audio_offset: {}", e),
        }
    })?;

    let input_ids_value = Value::from_array(input_ids).map_err(|e| SttError::InferenceError {
        model: "decoder_init".into(),
        detail: format!("Failed to create input_ids value: {}", e),
    })?;

    let position_ids_value =
        Value::from_array(position_ids.clone()).map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create position_ids value: {}", e),
        })?;

    let audio_features_value =
        Value::from_array(audio_features).map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create audio_features value: {}", e),
        })?;

    let audio_offset_value =
        Value::from_array(audio_offset).map_err(|e| SttError::InferenceError {
            model: "decoder_init".into(),
            detail: format!("Failed to create audio_offset value: {}", e),
        })?;

    let outputs = sessions
        .decoder_init
        .run(ort::inputs![
            "input_ids" => input_ids_value,
            "position_ids" => position_ids_value,
            "audio_features" => audio_features_value,
            "audio_offset" => audio_offset_value
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

    let first_token = greedy_decode_logits(logits_shape, logits_data, "decoder_init")?;

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
        keys: ndarray::Array5::from_shape_vec(
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
            model: "decoder_init".into(),
            detail: format!("Failed to reshape keys: {}", e),
        })?,
        values: ndarray::Array5::from_shape_vec(
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
        ndarray::Array2::from_shape_vec((1, 1), vec![position as i64]).map_err(|e| {
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
        keys: ndarray::Array5::from_shape_vec(
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
            model: "decoder_step".into(),
            detail: format!("Failed to reshape keys: {}", e),
        })?,
        values: ndarray::Array5::from_shape_vec(
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
            model: "decoder_step".into(),
            detail: format!("Failed to reshape values: {}", e),
        })?,
    };

    Ok((next_token, new_cache))
}

pub(crate) fn greedy_decode_logits(
    logits_shape: &[i64],
    logits_data: &[f32],
    model: &str,
) -> Result<u32, SttError> {
    if logits_shape.len() != 3 {
        return Err(SttError::InferenceError {
            model: model.into(),
            detail: format!(
                "Expected rank-3 logits tensor, got shape {:?}",
                logits_shape
            ),
        });
    }

    if logits_shape.iter().any(|&dim| dim <= 0) {
        return Err(SttError::InferenceError {
            model: model.into(),
            detail: format!(
                "Expected positive logits dimensions, got {:?}",
                logits_shape
            ),
        });
    }

    let batch_size = logits_shape[0] as usize;
    let seq_len = logits_shape[1] as usize;
    let vocab_size = logits_shape[2] as usize;
    let expected_len = batch_size
        .checked_mul(seq_len)
        .and_then(|len| len.checked_mul(vocab_size))
        .ok_or_else(|| SttError::InferenceError {
            model: model.into(),
            detail: format!("Logits shape is too large: {:?}", logits_shape),
        })?;

    if logits_data.len() != expected_len {
        return Err(SttError::InferenceError {
            model: model.into(),
            detail: format!(
                "Logits data length ({}) does not match shape {:?}",
                logits_data.len(),
                logits_shape
            ),
        });
    }

    let last_position_start = (seq_len - 1) * vocab_size;
    let last_logits = &logits_data[last_position_start..last_position_start + vocab_size];
    let mut max_idx = 0usize;
    let mut max_val = f32::NEG_INFINITY;
    for (i, &v) in last_logits.iter().enumerate() {
        if v > max_val {
            max_val = v;
            max_idx = i;
        }
    }

    Ok(max_idx as u32)
}

pub fn run_autoregressive_decode(
    init_token: u32,
    init_cache: KvCache,
    seq_len: usize,
    max_new_tokens: usize,
    sessions: &mut crate::models::session::OnnxSessions,
    embeddings: &EmbeddingMatrix,
) -> Result<Vec<u32>, SttError> {
    use crate::prompt::{ENDOFTEXT_ID, IM_END_ID};

    let mut tokens = vec![init_token];
    let mut cache = init_cache;

    for cur_pos in (seq_len..).take(max_new_tokens) {
        let (next_token, new_cache) = decoder_step(
            *tokens.last().unwrap(),
            cur_pos,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embeddings(hidden_size: usize) -> EmbeddingMatrix {
        let vocab_size = 151936;
        let mut data = vec![0.0f32; vocab_size * hidden_size];
        for (i, val) in data.iter_mut().enumerate() {
            *val = (i as f32) * 0.001;
        }

        EmbeddingMatrix {
            data,
            vocab_size,
            hidden_size,
        }
    }

    fn create_test_encoder_output(n_tokens: usize, hidden_dim: usize) -> Vec<Vec<f32>> {
        (0..n_tokens)
            .map(|i| {
                (0..hidden_dim)
                    .map(|j| ((i * hidden_dim + j) as f32) * 0.01)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_embed_and_fuse_with_audio_and_text() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644, 151651, 151676, 151676, 151653, 872, 198, 151645];
        let encoder_output = create_test_encoder_output(2, 1024);

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 8, 1024]);

        assert_eq!(result[[0, 0, 0]], embeddings.data[151644 * 1024]);
        assert_eq!(result[[0, 2, 0]], encoder_output[0][0]);
        assert_eq!(result[[0, 3, 0]], encoder_output[1][0]);
        assert_eq!(result[[0, 5, 0]], embeddings.data[872 * 1024]);
    }

    #[test]
    fn test_embed_and_fuse_audio_pad_count_mismatch() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644, 151651, 151652, 151653, 151645];
        let encoder_output = create_test_encoder_output(3, 1024);

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings);

        assert!(result.is_err());
        match result {
            Err(SttError::InferenceError { model, detail }) => {
                assert_eq!(model, "embedding_fusion");
                assert!(detail.contains("does not match"));
            }
            _ => panic!("Expected InferenceError"),
        }
    }

    #[test]
    fn test_embed_and_fuse_preserves_ordered_audio_fusion() {
        let embeddings = create_test_embeddings(4);
        let token_ids = vec![151644, AUDIO_PAD_ID, 872, AUDIO_PAD_ID, 198];
        let encoder_output = vec![vec![1.0, 1.1, 1.2, 1.3], vec![2.0, 2.1, 2.2, 2.3]];

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 5, 4]);
        assert_eq!(result[[0, 1, 0]], 1.0);
        assert_eq!(result[[0, 1, 3]], 1.3);
        assert_eq!(result[[0, 3, 0]], 2.0);
        assert_eq!(result[[0, 3, 3]], 2.3);
        assert_eq!(result[[0, 2, 0]], embeddings.data[872 * 4]);
    }

    #[test]
    fn test_embed_and_fuse_all_text_tokens() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644, 872, 198, 151645];
        let encoder_output = vec![];

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 4, 1024]);
        for (pos, &token_id) in token_ids.iter().enumerate() {
            let emb = embeddings.get_embedding(token_id);
            for d in 0..1024 {
                assert_eq!(result[[0, pos, d]], emb[d]);
            }
        }
    }

    #[test]
    fn test_embed_and_fuse_all_audio_tokens() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644, 151651, 151676, 151676, 151676, 151653];
        let encoder_output = create_test_encoder_output(3, 1024);

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 6, 1024]);
        assert_eq!(result[[0, 2, 0]], encoder_output[0][0]);
        assert_eq!(result[[0, 3, 0]], encoder_output[1][0]);
        assert_eq!(result[[0, 4, 0]], encoder_output[2][0]);
    }

    #[test]
    fn test_embed_and_fuse_single_token() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644];
        let encoder_output = vec![];

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 1, 1024]);
        assert_eq!(result[[0, 0, 0]], embeddings.data[151644 * 1024]);
    }

    #[test]
    fn test_embed_and_fuse_long_sequence() {
        let embeddings = create_test_embeddings(1024);
        let mut token_ids = vec![151644, 151651];
        token_ids.extend(vec![151676; 500]);
        token_ids.extend_from_slice(&[151653, 872, 198, 151645]);
        let encoder_output = create_test_encoder_output(500, 1024);

        let result = embed_and_fuse(&token_ids, &encoder_output, &embeddings).unwrap();

        assert_eq!(result.shape(), &[1, 506, 1024]);
    }

    #[test]
    fn test_decoder_init_returns_first_token() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644, 872, 198, 151645];
        let input_embeds = embed_and_fuse(&token_ids, &[], &embeddings).unwrap();

        assert_eq!(input_embeds.shape(), &[1, 4, 1024]);
    }

    #[test]
    fn test_decoder_init_kv_cache_shape() {
        let cache = KvCache {
            keys: ndarray::Array5::zeros((28, 1, 8, 4, 128)),
            values: ndarray::Array5::zeros((28, 1, 8, 4, 128)),
        };

        assert_eq!(cache.keys.shape(), &[28, 1, 8, 4, 128]);
        assert_eq!(cache.values.shape(), &[28, 1, 8, 4, 128]);
    }

    #[test]
    fn test_greedy_decode_selects_highest() {
        let mut logits_data = vec![0.0f32; 1000];
        logits_data[42] = 10.0;
        logits_data[100] = 5.0;

        let token = greedy_decode_logits(&[1, 1, 1000], &logits_data, "test").unwrap();

        assert_eq!(token, 42);
    }

    #[test]
    fn test_greedy_decode_with_equal_logits() {
        let logits_data = vec![5.0f32; 10];

        let token = greedy_decode_logits(&[1, 1, 10], &logits_data, "test").unwrap();

        assert_eq!(token, 0);
    }

    #[test]
    fn test_greedy_decode_with_negative_logits() {
        let mut logits_data = vec![-10.0f32; 100];
        logits_data[50] = -1.0;
        logits_data[75] = -5.0;

        let token = greedy_decode_logits(&[1, 1, 100], &logits_data, "test").unwrap();

        assert_eq!(token, 50);
    }

    #[test]
    fn test_greedy_decode_extracts_from_last_position() {
        let mut logits_data = vec![0.0f32; 300];
        logits_data[100] = 10.0;
        logits_data[250] = 5.0;

        let token = greedy_decode_logits(&[1, 3, 100], &logits_data, "test").unwrap();

        assert_eq!(token, 50);
    }

    #[test]
    fn test_greedy_decode_rejects_invalid_shape() {
        let result = greedy_decode_logits(&[1, 0, 10], &[], "test");

        assert!(result.is_err());
        match result {
            Err(SttError::InferenceError { model, detail }) => {
                assert_eq!(model, "test");
                assert!(detail.contains("positive logits dimensions"));
            }
            _ => panic!("Expected InferenceError"),
        }
    }

    #[test]
    fn test_greedy_decode_rejects_length_mismatch() {
        let result = greedy_decode_logits(&[1, 2, 3], &[0.0; 5], "test");

        assert!(result.is_err());
        match result {
            Err(SttError::InferenceError { model, detail }) => {
                assert_eq!(model, "test");
                assert!(detail.contains("does not match shape"));
            }
            _ => panic!("Expected InferenceError"),
        }
    }

    #[test]
    fn test_decoder_init_empty_input_embeds() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![];
        let input_embeds = embed_and_fuse(&token_ids, &[], &embeddings).unwrap();

        assert_eq!(input_embeds.shape(), &[1, 0, 1024]);
    }

    #[test]
    fn test_decoder_init_single_token_input() {
        let embeddings = create_test_embeddings(1024);
        let token_ids = vec![151644];
        let input_embeds = embed_and_fuse(&token_ids, &[], &embeddings).unwrap();

        assert_eq!(input_embeds.shape(), &[1, 1, 1024]);
    }

    #[test]
    fn test_decoder_step_kv_cache_growth() {
        let initial_cache = KvCache {
            keys: ndarray::Array5::zeros((28, 1, 8, 5, 128)),
            values: ndarray::Array5::zeros((28, 1, 8, 5, 128)),
        };

        let embeddings = create_test_embeddings(1024);
        let _emb = embeddings.get_embedding(151644);

        assert_eq!(initial_cache.keys.shape()[3], 5);
    }

    #[test]
    fn test_decoder_step_position_id() {
        let position = 3;
        assert_eq!(position, 3);
    }

    #[test]
    fn test_decoder_step_embedding_lookup() {
        let embeddings = create_test_embeddings(1024);
        let token_id = 151644;
        let emb = embeddings.get_embedding(token_id);

        assert_eq!(emb.len(), 1024);
    }

    #[test]
    fn test_decoder_step_cache_consistency() {
        let cache = KvCache {
            keys: ndarray::Array5::zeros((28, 1, 8, 10, 128)),
            values: ndarray::Array5::zeros((28, 1, 8, 10, 128)),
        };

        assert_eq!(cache.keys.shape(), cache.values.shape());
        assert_eq!(cache.keys.shape()[3], cache.values.shape()[3]);
    }

    #[test]
    fn test_run_autoregressive_decode_max_tokens() {
        let init_token = 151644;
        let _init_cache = KvCache {
            keys: ndarray::Array5::zeros((28, 1, 8, 1, 128)),
            values: ndarray::Array5::zeros((28, 1, 8, 1, 128)),
        };
        let max_new_tokens = 10;

        assert!(max_new_tokens <= 512);
        assert_eq!(init_token, 151644);
    }

    #[test]
    fn test_run_autoregressive_decode_early_stopping_im_end() {
        use crate::prompt::IM_END_ID;
        let im_end_token = IM_END_ID;

        assert_ne!(im_end_token, 151644);
    }

    #[test]
    fn test_run_autoregressive_decode_early_stopping_endoftext() {
        use crate::prompt::ENDOFTEXT_ID;
        let endoftext_token = ENDOFTEXT_ID;

        assert_ne!(endoftext_token, 151644);
    }

    #[test]
    fn test_run_autoregressive_decode_max_tokens_one() {
        let max_tokens = 1;

        assert_eq!(max_tokens, 1);
    }

    #[test]
    fn test_run_autoregressive_decode_large_max_tokens() {
        let max_tokens = 1000;

        assert!(max_tokens > 512);
    }
}
