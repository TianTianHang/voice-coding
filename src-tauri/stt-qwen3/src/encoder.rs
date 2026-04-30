use ort::value::Value;

use stt_core::SttError;

pub fn run_encoder(
    mel: &[Vec<f64>],
    sessions: &mut crate::models::session::OnnxSessions,
) -> Result<Vec<Vec<f32>>, SttError> {
    let n_mels = mel.len();
    let n_frames = mel.first().map(|row| row.len()).unwrap_or(0);

    let input_tensor = ndarray::Array3::from_shape_vec(
        (1, n_mels, n_frames),
        mel.iter()
            .flat_map(|row| row.iter().map(|&v| v as f32))
            .collect(),
    )
    .map_err(|e| SttError::InferenceError {
        model: "encoder".into(),
        detail: format!("Failed to create input tensor: {}", e),
    })?;

    let input_value = Value::from_array(input_tensor).map_err(|e| SttError::InferenceError {
        model: "encoder".into(),
        detail: format!("Failed to create input value: {}", e),
    })?;

    let outputs = sessions
        .encoder
        .run(ort::inputs!["mel" => input_value])
        .map_err(|e| SttError::InferenceError {
            model: "encoder".into(),
            detail: format!("Inference failed: {}", e),
        })?;

    let audio_features = &outputs[0];
    let (shape, data) =
        audio_features
            .try_extract_tensor::<f32>()
            .map_err(|e| SttError::InferenceError {
                model: "encoder".into(),
                detail: format!("Failed to extract output: {}", e),
            })?;

    let batch = shape[0] as usize;
    let seq_len = shape[1] as usize;
    let hidden = shape[2] as usize;
    if batch != 1 {
        return Err(SttError::InferenceError {
            model: "encoder".into(),
            detail: format!("Expected batch size 1, got {}", batch),
        });
    }

    let mut result = Vec::with_capacity(seq_len);
    for row in data.chunks(hidden).take(seq_len) {
        result.push(row.to_vec());
    }

    Ok(result)
}
