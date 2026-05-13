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
    #[allow(dead_code)]
    #[serde(default = "default_audio_temperature")]
    audio_temperature: f32,
}

fn default_audio_temperature() -> f32 {
    0.8
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
    #[allow(dead_code)]
    #[serde(default)]
    model_config: TtsModelConfig,
    onnx: TtsOnnxMeta,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct TtsModelConfig {
    #[allow(dead_code)]
    #[serde(default)]
    local_layers: usize,
    #[allow(dead_code)]
    #[serde(default)]
    local_heads: usize,
    #[allow(dead_code)]
    #[serde(default)]
    local_head_dim: usize,
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
    #[serde(default)]
    streaming_decode: Option<StreamingDecodeMeta>,
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
    encode_input_names: Vec<String>,
    #[serde(default)]
    encode_output_names: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    decode_input_names: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    decode_output_names: Vec<String>,
    #[serde(default)]
    decode_step_input_names: Vec<String>,
    #[serde(default)]
    decode_step_output_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StreamingDecodeMeta {
    #[serde(default = "default_decode_step_batch_size")]
    batch_size: usize,
    #[serde(default)]
    transformer_offsets: Vec<TransformerOffsetMeta>,
    #[serde(default)]
    attention_caches: Vec<AttentionCacheMeta>,
}

#[derive(Debug, Clone, Deserialize)]
struct TransformerOffsetMeta {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    dtype: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AttentionCacheMeta {
    offset_input_name: String,
    offset_output_name: String,
    cached_keys_input_name: String,
    cached_keys_output_name: String,
    cached_values_input_name: String,
    cached_values_output_name: String,
    cached_positions_input_name: String,
    cached_positions_output_name: String,
    offset_shape: Vec<usize>,
    cache_shape: Vec<usize>,
    positions_shape: Vec<usize>,
    cache_dtype: String,
    positions_dtype: String,
}

fn default_decode_step_batch_size() -> usize {
    1
}

#[derive(Debug, Clone)]
struct CodecDecodeStepState {
    batch_size: usize,
    transformer_offsets: Vec<NamedI32TensorState>,
    attention_caches: Vec<AttentionCacheState>,
}

#[derive(Debug, Clone)]
struct NamedI32TensorState {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    data: Vec<i32>,
}

#[derive(Debug, Clone)]
struct NamedF32TensorState {
    input_name: String,
    output_name: String,
    shape: Vec<usize>,
    data: Vec<f32>,
}

#[derive(Debug, Clone)]
struct AttentionCacheState {
    offset: NamedI32TensorState,
    keys: NamedF32TensorState,
    values: NamedF32TensorState,
    positions: NamedI32TensorState,
}

impl CodecDecodeStepState {
    fn from_meta(meta: &CodecMeta) -> Result<Self, MossTtsError> {
        let streaming = meta.streaming_decode.as_ref().ok_or_else(|| {
            MossTtsError::MetadataMismatch(
                "codec streaming_decode metadata is required for decode_step".to_string(),
            )
        })?;
        if streaming.batch_size == 0 {
            return Err(MossTtsError::MetadataMismatch(
                "codec streaming_decode.batch_size must be greater than zero".to_string(),
            ));
        }

        let transformer_offsets = streaming
            .transformer_offsets
            .iter()
            .map(|offset| {
                ensure_dtype(&offset.dtype, "int32", &offset.input_name)?;
                Ok(NamedI32TensorState::zeros(
                    offset.input_name.clone(),
                    offset.output_name.clone(),
                    offset.shape.clone(),
                ))
            })
            .collect::<Result<Vec<_>, MossTtsError>>()?;

        let attention_caches = streaming
            .attention_caches
            .iter()
            .map(|cache| {
                ensure_dtype("int32", "int32", &cache.offset_input_name)?;
                ensure_dtype(&cache.cache_dtype, "float32", &cache.cached_keys_input_name)?;
                ensure_dtype(&cache.cache_dtype, "float32", &cache.cached_values_input_name)?;
                ensure_dtype(
                    &cache.positions_dtype,
                    "int32",
                    &cache.cached_positions_input_name,
                )?;
                Ok(AttentionCacheState {
                    offset: NamedI32TensorState::zeros(
                        cache.offset_input_name.clone(),
                        cache.offset_output_name.clone(),
                        cache.offset_shape.clone(),
                    ),
                    keys: NamedF32TensorState::zeros(
                        cache.cached_keys_input_name.clone(),
                        cache.cached_keys_output_name.clone(),
                        cache.cache_shape.clone(),
                    ),
                    values: NamedF32TensorState::zeros(
                        cache.cached_values_input_name.clone(),
                        cache.cached_values_output_name.clone(),
                        cache.cache_shape.clone(),
                    ),
                    positions: NamedI32TensorState::zeros(
                        cache.cached_positions_input_name.clone(),
                        cache.cached_positions_output_name.clone(),
                        cache.positions_shape.clone(),
                    )
                    .with_fill(-1),
                })
            })
            .collect::<Result<Vec<_>, MossTtsError>>()?;

        Ok(Self {
            batch_size: streaming.batch_size,
            transformer_offsets,
            attention_caches,
        })
    }

    #[cfg(test)]
    fn input_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for offset in &self.transformer_offsets {
            names.push(offset.input_name.as_str());
        }
        for cache in &self.attention_caches {
            names.push(cache.offset.input_name.as_str());
            names.push(cache.keys.input_name.as_str());
            names.push(cache.values.input_name.as_str());
            names.push(cache.positions.input_name.as_str());
        }
        names
    }

    #[cfg(test)]
    fn output_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for offset in &self.transformer_offsets {
            names.push(offset.output_name.as_str());
        }
        for cache in &self.attention_caches {
            names.push(cache.offset.output_name.as_str());
            names.push(cache.keys.output_name.as_str());
            names.push(cache.values.output_name.as_str());
            names.push(cache.positions.output_name.as_str());
        }
        names
    }

    fn update_from_owned_outputs(
        &mut self,
        outputs: &mut HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        for offset in &mut self.transformer_offsets {
            offset.update_from_outputs(outputs)?;
        }
        for cache in &mut self.attention_caches {
            cache.offset.update_from_outputs(outputs)?;
            cache.keys.update_from_outputs(outputs)?;
            cache.values.update_from_outputs(outputs)?;
            cache.positions.update_from_outputs(outputs)?;
        }
        Ok(())
    }

    fn collect_input_views<'a>(
        &'a self,
        i32_tensors: &mut Vec<(String, ArrayViewD<'a, i32>)>,
        f32_tensors: &mut Vec<(String, ArrayViewD<'a, f32>)>,
    ) -> Result<(), MossTtsError> {
        for offset in &self.transformer_offsets {
            i32_tensors.push((offset.input_name.clone(), offset.view()?));
        }
        for cache in &self.attention_caches {
            i32_tensors.push((cache.offset.input_name.clone(), cache.offset.view()?));
            f32_tensors.push((cache.keys.input_name.clone(), cache.keys.view()?));
            f32_tensors.push((cache.values.input_name.clone(), cache.values.view()?));
            i32_tensors.push((cache.positions.input_name.clone(), cache.positions.view()?));
        }
        Ok(())
    }
}

impl NamedI32TensorState {
    fn zeros(input_name: String, output_name: String, shape: Vec<usize>) -> Self {
        let len = tensor_len(&shape);
        Self {
            input_name,
            output_name,
            shape,
            data: vec![0; len],
        }
    }

    fn with_fill(mut self, value: i32) -> Self {
        self.data.fill(value);
        self
    }

    fn update_from_outputs(
        &mut self,
        outputs: &mut HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        let tensor = outputs
            .remove(&self.output_name)
            .ok_or_else(|| codec_decode_step_unavailable(format!(
                "missing state output '{}'",
                self.output_name
            )))?;
        let OwnedTensorData::I32 { shape, data } = tensor else {
            return Err(codec_decode_step_unavailable(format!(
                "state output '{}' must be int32",
                self.output_name
            )));
        };
        self.shape = shape;
        self.data = data;
        Ok(())
    }

    fn view(&self) -> Result<ArrayViewD<'_, i32>, MossTtsError> {
        ArrayViewD::from_shape(IxDyn(&self.shape), &self.data).map_err(|e| {
            codec_decode_step_unavailable(format!(
                "failed to build state tensor view '{}': {e}",
                self.input_name
            ))
        })
    }
}

impl NamedF32TensorState {
    fn zeros(input_name: String, output_name: String, shape: Vec<usize>) -> Self {
        let len = tensor_len(&shape);
        Self {
            input_name,
            output_name,
            shape,
            data: vec![0.0; len],
        }
    }

    fn update_from_outputs(
        &mut self,
        outputs: &mut HashMap<String, OwnedTensorData>,
    ) -> Result<(), MossTtsError> {
        let tensor = outputs
            .remove(&self.output_name)
            .ok_or_else(|| codec_decode_step_unavailable(format!(
                "missing state output '{}'",
                self.output_name
            )))?;
        let OwnedTensorData::F32 { shape, data } = tensor else {
            return Err(codec_decode_step_unavailable(format!(
                "state output '{}' must be float32",
                self.output_name
            )));
        };
        self.shape = shape;
        self.data = data;
        Ok(())
    }

    fn view(&self) -> Result<ArrayViewD<'_, f32>, MossTtsError> {
        ArrayViewD::from_shape(IxDyn(&self.shape), &self.data).map_err(|e| {
            codec_decode_step_unavailable(format!(
                "failed to build state tensor view '{}': {e}",
                self.input_name
            ))
        })
    }
}

#[derive(Debug, Clone)]
enum OwnedTensorData {
    I32 { shape: Vec<usize>, data: Vec<i32> },
    F32 { shape: Vec<usize>, data: Vec<f32> },
}

fn tensor_len(shape: &[usize]) -> usize {
    shape.iter().copied().product::<usize>().max(1)
}

fn ensure_dtype(actual: &str, expected: &str, name: &str) -> Result<(), MossTtsError> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(MossTtsError::MetadataMismatch(format!(
            "{name} expected dtype {expected}, got {actual}"
        )))
    }
}
