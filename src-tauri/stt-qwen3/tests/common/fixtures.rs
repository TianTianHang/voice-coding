pub struct MelSpectrogramFixture;
pub struct AudioSampleFixture;
pub struct TokenSequenceFixture;

impl MelSpectrogramFixture {
    pub fn simple(n_mels: usize, n_frames: usize) -> Vec<Vec<f64>> {
        (0..n_mels)
            .map(|i| {
                (0..n_frames)
                    .map(|j| (i * n_frames + j) as f64 * 0.01)
                    .collect()
            })
            .collect()
    }

    pub fn with_chunks(n_mels: usize, n_frames: usize, chunk_size: usize) -> Vec<Vec<f64>> {
        let mut mel = Self::simple(n_mels, n_frames);
        for (i, row) in mel.iter_mut().enumerate() {
            for (j, val) in row.iter_mut().enumerate() {
                let chunk_idx = j / chunk_size;
                *val = *val * (1.0 + chunk_idx as f64 * 0.1);
            }
        }
        mel
    }

    pub fn empty() -> Vec<Vec<f64>> {
        vec![]
    }

    pub fn single_frame(n_mels: usize) -> Vec<Vec<f64>> {
        Self::simple(n_mels, 1)
    }
}

impl AudioSampleFixture {
    pub fn silence(duration_samples: usize, sample_rate: u32) -> Vec<f32> {
        vec![0.0f32; duration_samples]
    }

    pub fn sine_wave(duration_samples: usize, sample_rate: u32, frequency: f32) -> Vec<f32> {
        (0..duration_samples)
            .map(|i| (2.0 * std::f32::consts::PI * frequency * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    pub fn multi_tone(duration_samples: usize, sample_rate: u32) -> Vec<f32> {
        (0..duration_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (440.0 * 2.0 * std::f32::consts::PI * t).sin() * 0.3
                    + (880.0 * 2.0 * std::f32::consts::PI * t).sin() * 0.2
            })
            .collect()
    }

    pub fn empty() -> Vec<f32> {
        vec![]
    }

    pub fn too_short() -> Vec<f32> {
        Self::sine_wave(100, 16000, 440.0)
    }
}

impl TokenSequenceFixture {
    pub fn simple_text() -> Vec<u32> {
        vec![151644, 872, 198, 151645]
    }

    pub fn with_audio(n_audio_tokens: usize) -> Vec<u32> {
        let mut tokens = vec![151644];
        tokens.extend_from_slice(&[151651]);
        tokens.extend(vec![151652; n_audio_tokens]);
        tokens.extend_from_slice(&[151653, 151645]);
        tokens
    }

    pub fn only_audio(n_audio_tokens: usize) -> Vec<u32> {
        vec![
            vec![151644, 151651],
            vec![151652; n_audio_tokens],
            vec![151653, 151645],
        ]
        .concat()
    }

    pub fn only_text() -> Vec<u32> {
        vec![151644, 872, 198, 151645]
    }

    pub fn single_token() -> Vec<u32> {
        vec![151644]
    }

    pub fn empty() -> Vec<u32> {
        vec![]
    }
}
