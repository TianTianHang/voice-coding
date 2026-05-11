use crate::Result;

pub type KvCache = (ndarray::Array5<f32>, ndarray::Array5<f32>);

pub trait SessionManager: Send + Sync {
    fn run_encoder(&mut self, mel: &[Vec<f64>]) -> Result<Vec<Vec<f32>>>;

    fn run_decoder_init(
        &mut self,
        prompt_ids: &[u32],
        encoder_output: &[Vec<f32>],
    ) -> Result<(u32, KvCache)>;

    fn run_decoder_step(
        &mut self,
        input_embeds: &ndarray::Array3<f32>,
        position_ids: &ndarray::Array2<i64>,
        past_keys: &ndarray::Array5<f32>,
        past_values: &ndarray::Array5<f32>,
    ) -> Result<(u32, KvCache)>;
}
