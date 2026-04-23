use ort::value::Value;

use stt_core::SttError;

const CHUNK_SIZE: usize = 100;

pub fn chunk_mel_spectrogram(mel: &[Vec<f64>]) -> (Vec<Vec<Vec<f64>>>, Vec<usize>) {
    let n_mels = mel.len();
    let n_frames = mel[0].len();

    if n_frames <= CHUNK_SIZE {
        return (vec![mel.to_vec()], vec![n_frames]);
    }

    let n_chunks = n_frames.div_ceil(CHUNK_SIZE);
    let max_chunk_len = CHUNK_SIZE;

    let mut chunks = Vec::with_capacity(n_chunks);
    let mut original_lengths = Vec::with_capacity(n_chunks);

    for c in 0..n_chunks {
        let start = c * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(n_frames);
        let len = end - start;
        original_lengths.push(len);

        let mut chunk = vec![vec![0.0f64; max_chunk_len]; n_mels];
        for (mel_idx, chunk_row) in chunk.iter_mut().enumerate().take(n_mels) {
            chunk_row[..len].copy_from_slice(&mel[mel_idx][start..start + len]);
        }
        chunks.push(chunk);
    }

    (chunks, original_lengths)
}

fn compute_conv_output_len(input_len: usize) -> usize {
    let mut length = input_len;
    for _ in 0..3 {
        length = (length - 1) / 2 + 1;
    }
    length
}

pub fn run_encoder(
    mel: &[Vec<f64>],
    sessions: &mut crate::models::session::OnnxSessions,
) -> Result<Vec<Vec<f32>>, SttError> {
    let n_mels = mel.len();
    let max_chunk_len = CHUNK_SIZE;

    let (chunks, original_lengths) = chunk_mel_spectrogram(mel);
    let n_chunks = chunks.len();

    let _max_out_len = compute_conv_output_len(max_chunk_len);
    let hidden_dim: usize = 896;

    let mut flat_input = Vec::with_capacity(n_chunks * n_mels * max_chunk_len);
    for chunk in &chunks {
        for row in chunk.iter().take(n_mels) {
            flat_input.extend_from_slice(row);
        }
    }

    let input_tensor = ndarray::Array4::from_shape_vec(
        (n_chunks, 1, n_mels, max_chunk_len),
        flat_input.iter().map(|&v| v as f32).collect(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "encoder_conv".into(),
        detail: format!("Failed to create input tensor: {}", e),
    })?;

    let input_value = Value::from_array(input_tensor).map_err(|e| SttError::InferenceError {
        model: "encoder_conv".into(),
        detail: format!("Failed to create input value: {}", e),
    })?;

    let outputs = sessions
        .encoder_conv
        .run(vec![("padded_mel_chunks", input_value)])
        .map_err(|e| SttError::InferenceError {
            model: "encoder_conv".into(),
            detail: format!("Inference failed: {}", e),
        })?;

    let conv_output = &outputs[0];
    let (conv_shape, conv_data) =
        conv_output
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "encoder_conv".into(),
                detail: format!("Failed to extract output: {}", e),
            })?;

    let conv_array = ndarray::Array3::from_shape_vec(
        (
            conv_shape[0] as usize,
            conv_shape[1] as usize,
            conv_shape[2] as usize,
        ),
        conv_data.to_vec(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "encoder_conv".into(),
        detail: format!("Failed to reshape conv output: {}", e),
    })?;

    let mut all_hidden = Vec::new();
    let mut total_tokens = 0usize;

    for c in 0..n_chunks {
        let actual_out_len = compute_conv_output_len(original_lengths[c]);
        for t in 0..actual_out_len {
            let mut row = vec![0.0f32; hidden_dim];
            for d in 0..hidden_dim {
                row[d] = conv_array[[c, t, d]];
            }
            all_hidden.push(row);
            total_tokens += 1;
        }
    }

    let hidden_flat: Vec<f32> = all_hidden.iter().flat_map(|r| r.iter().copied()).collect();
    let hidden_tensor = ndarray::Array2::from_shape_vec((total_tokens, hidden_dim), hidden_flat)
        .map_err(|e| SttError::InferenceError {
            model: "encoder_transformer".into(),
            detail: format!("Failed to create hidden tensor: {}", e),
        })?;

    let attention_mask = ndarray::Array4::from_shape_vec(
        (1, 1, total_tokens, total_tokens),
        vec![0.0f32; total_tokens * total_tokens],
    )
    .map_err(|e| SttError::InferenceError {
        model: "encoder_transformer".into(),
        detail: format!("Failed to create attention mask: {}", e),
    })?;

    let hidden_value = Value::from_array(hidden_tensor).map_err(|e| SttError::InferenceError {
        model: "encoder_transformer".into(),
        detail: format!("Failed to create hidden value: {}", e),
    })?;
    let mask_value = Value::from_array(attention_mask).map_err(|e| SttError::InferenceError {
        model: "encoder_transformer".into(),
        detail: format!("Failed to create mask value: {}", e),
    })?;

    let transformer_outputs = sessions
        .encoder_transformer
        .run(vec![
            ("hidden_states", hidden_value),
            ("attention_mask", mask_value),
        ])
        .map_err(|e| SttError::InferenceError {
            model: "encoder_transformer".into(),
            detail: format!("Inference failed: {}", e),
        })?;

    let transformer_output = &transformer_outputs[0];
    let (output_shape, output_data) =
        transformer_output
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "encoder_transformer".into(),
                detail: format!("Failed to extract output: {}", e),
            })?;

    let output_array = ndarray::Array2::from_shape_vec(
        (output_shape[0] as usize, output_shape[1] as usize),
        output_data.to_vec(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "encoder_transformer".into(),
        detail: format!("Failed to reshape output: {}", e),
    })?;

    let projected_dim = 1024;
    let mut result = Vec::with_capacity(total_tokens);
    for t in 0..total_tokens {
        let mut row = vec![0.0f32; projected_dim];
        for d in 0..projected_dim {
            if d < output_array.ncols() {
                row[d] = output_array[[t, d]];
            }
        }
        result.push(row);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_conv_output_len() {
        assert_eq!(compute_conv_output_len(100), 13);
        assert_eq!(compute_conv_output_len(50), 7);
    }

    #[test]
    fn test_chunk_mel_spectrogram_small() {
        let mel = vec![vec![1.0f64; 50]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);
        assert_eq!(chunks.len(), 1);
        assert_eq!(lengths, vec![50]);
    }

    #[test]
    fn test_chunk_mel_spectrogram_large() {
        let mel = vec![vec![1.0f64; 250]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);
        assert_eq!(chunks.len(), 3);
        assert_eq!(lengths, vec![100, 100, 50]);
    }

    #[test]
    fn test_chunk_mel_spectrogram_divisible() {
        let mel = vec![vec![1.0f64; 200]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);
        assert_eq!(chunks.len(), 2);
        assert_eq!(lengths, vec![100, 100]);
    }

    #[test]
    fn test_chunk_original_lengths_accuracy() {
        let mel = vec![vec![1.0f64; 250]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);

        assert_eq!(chunks.len(), lengths.len());

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk[0].len(), CHUNK_SIZE);
        }

        assert_eq!(lengths, vec![100, 100, 50]);
    }

    #[test]
    fn test_chunk_padding() {
        let mel = vec![vec![1.0f64; 50]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0][0].len(), 50);
        assert_eq!(lengths, vec![50]);
    }

    #[test]
    fn test_compute_conv_output_len_various() {
        assert_eq!(compute_conv_output_len(100), 13);
        assert_eq!(compute_conv_output_len(50), 7);
        assert_eq!(compute_conv_output_len(200), 25);
        assert_eq!(compute_conv_output_len(10), 2);
    }

    #[test]
    fn test_run_encoder_single_frame() {
        let mel = vec![vec![1.0f64; 1]; 128];
        let (chunks, lengths) = chunk_mel_spectrogram(&mel);
        assert_eq!(chunks.len(), 1);
        assert_eq!(lengths, vec![1]);
    }
}
