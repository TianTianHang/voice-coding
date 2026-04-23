use ndarray::{Array2, Array3, Array5};
use stt_core::{KvCache, Result, SessionManager, SttError};

#[derive(Debug, Clone)]
pub struct MockSessionManager {
    pub encoder_output: Option<Vec<Vec<f32>>>,
    pub decoder_init_output: Option<(u32, KvCache)>,
    pub decoder_step_output: Option<(u32, KvCache)>,
    pub should_fail: bool,
}

impl MockSessionManager {
    pub fn new() -> Self {
        Self {
            encoder_output: None,
            decoder_init_output: None,
            decoder_step_output: None,
            should_fail: false,
        }
    }

    pub fn with_encoder_output(mut self, output: Vec<Vec<f32>>) -> Self {
        self.encoder_output = Some(output);
        self
    }

    pub fn with_decoder_init_output(
        mut self,
        token: u32,
        keys: Array5<f32>,
        values: Array5<f32>,
    ) -> Self {
        self.decoder_init_output = Some((token, (keys, values)));
        self
    }

    pub fn with_decoder_step_output(
        mut self,
        token: u32,
        keys: Array5<f32>,
        values: Array5<f32>,
    ) -> Self {
        self.decoder_step_output = Some((token, (keys, values)));
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub fn default_encoder_output(n_tokens: usize, hidden_dim: usize) -> Vec<Vec<f32>> {
        (0..n_tokens)
            .map(|i| {
                (0..hidden_dim)
                    .map(|j| (i * hidden_dim + j) as f32)
                    .collect()
            })
            .collect()
    }

    pub fn default_cache(
        num_layers: usize,
        batch: usize,
        num_heads: usize,
        seq_len: usize,
        head_dim: usize,
    ) -> KvCache {
        let keys = Array5::from_shape_vec(
            (num_layers, batch, num_heads, seq_len, head_dim),
            vec![0.0f32; num_layers * batch * num_heads * seq_len * head_dim],
        )
        .unwrap();
        let values = Array5::from_shape_vec(
            (num_layers, batch, num_heads, seq_len, head_dim),
            vec![0.0f32; num_layers * batch * num_heads * seq_len * head_dim],
        )
        .unwrap();
        (keys, values)
    }
}

impl Default for MockSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager for MockSessionManager {
    fn run_encoder(&mut self, _mel: &[Vec<f64>]) -> Result<Vec<Vec<f32>>, SttError> {
        if self.should_fail {
            return Err(SttError::InferenceError {
                model: "mock_encoder".into(),
                detail: "Mock encoder failure".into(),
            });
        }

        Ok(self
            .encoder_output
            .clone()
            .unwrap_or_else(|| Self::default_encoder_output(100, 1024)))
    }

    fn run_decoder_init(
        &mut self,
        _input_embeds: &Array3<f32>,
        _position_ids: &Array2<i64>,
    ) -> Result<(u32, KvCache), SttError> {
        if self.should_fail {
            return Err(SttError::InferenceError {
                model: "mock_decoder_init".into(),
                detail: "Mock decoder_init failure".into(),
            });
        }

        Ok(self.decoder_init_output.clone().unwrap_or_else(|| {
            let cache = Self::default_cache(28, 1, 8, 1, 128);
            (151644, cache)
        }))
    }

    fn run_decoder_step(
        &mut self,
        _input_embeds: &Array3<f32>,
        _position_ids: &Array2<i64>,
        _past_keys: &Array5<f32>,
        _past_values: &Array5<f32>,
    ) -> Result<(u32, KvCache), SttError> {
        if self.should_fail {
            return Err(SttError::InferenceError {
                model: "mock_decoder_step".into(),
                detail: "Mock decoder_step failure".into(),
            });
        }

        Ok(self.decoder_step_output.clone().unwrap_or_else(|| {
            let cache = Self::default_cache(28, 1, 8, 2, 128);
            (151645, cache)
        }))
    }
}
