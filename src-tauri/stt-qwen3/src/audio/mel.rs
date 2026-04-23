const N_FFT: usize = 400;
const HOP_LENGTH: usize = 160;
pub const N_MELS: usize = 128;
const SAMPLE_RATE: u32 = 16000;
const F_MAX: f64 = 8000.0;

pub fn hann_window(size: usize) -> Vec<f64> {
    let mut window = Vec::with_capacity(size);
    for i in 0..size {
        let val = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (size - 1) as f64).cos());
        window.push(val);
    }
    window
}

pub fn compute_stft(samples: &[f32]) -> Vec<Vec<num_complex::Complex64>> {
    let n_frames = if samples.len() >= N_FFT {
        (samples.len() - N_FFT) / HOP_LENGTH + 1
    } else {
        1
    };
    let n_bins = N_FFT / 2 + 1;

    let window = hann_window(N_FFT);
    let mut planner = rustfft::FftPlanner::new();
    let fft = planner.plan_fft_forward(N_FFT);

    let mut stft_output = Vec::with_capacity(n_frames);

    for frame_idx in 0..n_frames {
        let start = frame_idx * HOP_LENGTH;
        let _end = (start + N_FFT).min(samples.len());

        let mut fft_input: Vec<num_complex::Complex64> = vec![num_complex::Complex64::new(0.0, 0.0); N_FFT];
        for i in 0..N_FFT {
            let sample_idx = start + i;
            let sample = if sample_idx < samples.len() {
                samples[sample_idx] as f64
            } else {
                0.0
            };
            let windowed = sample * window[i];
            fft_input[i] = num_complex::Complex64::new(windowed, 0.0);
        }

        fft.process(&mut fft_input);

        stft_output.push(fft_input[..n_bins].to_vec());
    }

    stft_output
}

fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (mel / 1127.0).exp() - 700.0
}

fn mel_frequencies(n_mels: usize) -> Vec<f64> {
    let mel_min = 0.0;
    let mel_max = 1127.0 * (1.0 + F_MAX / 700.0).ln();
    let step = (mel_max - mel_min) / (n_mels + 1) as f64;
    (0..=n_mels + 1)
        .map(|i| mel_to_hz(mel_min + i as f64 * step))
        .collect()
}

pub fn create_mel_filterbank() -> Vec<Vec<f64>> {
    let n_bins = N_FFT / 2 + 1;
    let fft_freqs: Vec<f64> = (0..n_bins)
        .map(|i| i as f64 * SAMPLE_RATE as f64 / N_FFT as f64)
        .collect();

    let mel_points = mel_frequencies(N_MELS);
    let mut filterbank = vec![vec![0.0f64; n_bins]; N_MELS];

    for i in 0..N_MELS {
        let f_left = mel_points[i];
        let f_center = mel_points[i + 1];
        let f_right = mel_points[i + 2];

        let enorm = 2.0 / (f_right - f_left);

        for j in 0..n_bins {
            let f = fft_freqs[j];
            if f >= f_left && f <= f_center && f_center > f_left {
                filterbank[i][j] = enorm * (f - f_left) / (f_center - f_left);
            } else if f >= f_center && f <= f_right && f_right > f_center {
                filterbank[i][j] = enorm * (f_right - f) / (f_right - f_center);
            }
        }
    }

    filterbank
}

pub fn apply_mel_filterbank(
    stft_output: &[Vec<num_complex::Complex64>],
    filterbank: &[Vec<f64>],
) -> Vec<Vec<f64>> {
    let n_frames = stft_output.len();
    let n_bins = stft_output[0].len();

    let mut power_spec = vec![vec![0.0f64; n_frames]; n_bins];
    for (frame_idx, frame) in stft_output.iter().enumerate() {
        for (bin_idx, c) in frame.iter().enumerate() {
            let magnitude = (c.re * c.re + c.im * c.im).sqrt();
            power_spec[bin_idx][frame_idx] = magnitude * magnitude;
        }
    }

    let mut mel_spec = vec![vec![0.0f64; n_frames]; N_MELS];
    for (mel_idx, filter) in filterbank.iter().enumerate() {
        for frame_idx in 0..n_frames {
            let mut sum = 0.0f64;
            for bin_idx in 0..n_bins {
                sum += filter[bin_idx] * power_spec[bin_idx][frame_idx];
            }
            mel_spec[mel_idx][frame_idx] = sum;
        }
    }

    mel_spec
}

pub fn log_compression(mel_spec: &mut [Vec<f64>]) {
    let mut max_val = f64::NEG_INFINITY;
    for row in mel_spec.iter() {
        for &v in row.iter() {
            let log_v = v.max(1e-10).log10();
            if log_v > max_val {
                max_val = log_v;
            }
        }
    }

    for row in mel_spec.iter_mut() {
        for v in row.iter_mut() {
            *v = v.max(1e-10).log10();
            *v = v.max(max_val - 8.0);
            *v = (*v + 4.0) / 4.0;
        }
    }
}

pub fn compute_mel_spectrogram(samples: &[f32], filterbank: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let stft_output = compute_stft(samples);
    let mut mel_spec = apply_mel_filterbank(&stft_output, filterbank);
    log_compression(&mut mel_spec);
    mel_spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_length() {
        let window = hann_window(400);
        assert_eq!(window.len(), 400);
    }

    #[test]
    fn test_hann_window_symmetry() {
        let window = hann_window(400);
        for i in 0..200 {
            assert!((window[i] - window[399 - i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_stft_output_shape() {
        let samples = vec![0.5f32; 16000];
        let stft = compute_stft(&samples);
        let expected_frames = (16000 - 400) / 160 + 1;
        assert_eq!(stft.len(), expected_frames);
        assert_eq!(stft[0].len(), 201);
    }

    #[test]
    fn test_mel_filterbank_shape() {
        let fb = create_mel_filterbank();
        assert_eq!(fb.len(), 128);
        assert_eq!(fb[0].len(), 201);
    }

    #[test]
    fn test_mel_spectrogram_output() {
        let samples = vec![0.5f32; 16000];
        let fb = create_mel_filterbank();
        let mel = compute_mel_spectrogram(&samples, &fb);
        assert_eq!(mel.len(), 128);
        assert!(mel[0].len() > 0);
    }

    #[test]
    fn test_filterbank_nonnegative() {
        let fb = create_mel_filterbank();
        for row in &fb {
            for &v in row {
                assert!(v >= 0.0);
            }
        }
    }

    #[test]
    fn test_log_compression_range() {
        let mut mel = vec![vec![1e-5f64; 10]; 128];
        log_compression(&mut mel);
        for row in &mel {
            for &v in row {
                assert!(v >= -1.0 && v <= 2.0);
            }
        }
    }
}
