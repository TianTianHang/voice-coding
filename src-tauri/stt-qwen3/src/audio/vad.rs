const SAMPLE_RATE: u32 = 16000;
const VAD_FRAME_LENGTH_SEC: f64 = 0.2;
const VAD_HOP_SEC: f64 = 0.1;
const SILENCE_THRESHOLD_DB: f64 = -40.0;
const MIN_DURATION_FOR_CHUNKING_SEC: f64 = 45.0;

pub fn compute_rms_energy(samples: &[f32], frame_length: usize, hop: usize) -> Vec<f64> {
    let mut rms_values = Vec::new();
    let mut pos = 0;
    while pos + frame_length <= samples.len() {
        let frame = &samples[pos..pos + frame_length];
        let sum_sq: f64 = frame.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum_sq / frame_length as f64).sqrt();
        rms_values.push(rms);
        pos += hop;
    }
    rms_values
}

pub fn rms_to_db(rms_values: &[f64]) -> Vec<f64> {
    let max_rms = rms_values.iter().cloned().fold(0.0f64, f64::max);
    if max_rms <= 0.0 {
        return vec![f64::NEG_INFINITY; rms_values.len()];
    }
    rms_values
        .iter()
        .map(|&rms| 20.0 * (rms.max(1e-10) / max_rms).log10())
        .collect()
}

pub fn detect_silence(db_values: &[f64], threshold: f64) -> Vec<bool> {
    db_values.iter().map(|&db| db < threshold).collect()
}

pub fn find_split_points(samples: &[f32], target_chunk_sec: f64) -> Vec<usize> {
    let duration = samples.len() as f64 / SAMPLE_RATE as f64;
    if duration < MIN_DURATION_FOR_CHUNKING_SEC {
        return Vec::new();
    }

    let frame_length = (SAMPLE_RATE as f64 * VAD_FRAME_LENGTH_SEC) as usize;
    let hop = (SAMPLE_RATE as f64 * VAD_HOP_SEC) as usize;
    let target_samples = (target_chunk_sec * SAMPLE_RATE as f64) as usize;

    let rms_values = compute_rms_energy(samples, frame_length, hop);
    let db_values = rms_to_db(&rms_values);
    let silence_mask = detect_silence(&db_values, SILENCE_THRESHOLD_DB);

    let mut split_points = Vec::new();
    let mut current_pos = 0usize;
    let min_chunk = (target_samples as f64 * 0.5) as usize;
    let max_chunk = (target_samples as f64 * 1.5) as usize;

    while current_pos + max_chunk < samples.len() {
        let search_start = current_pos + min_chunk;
        let search_end = (current_pos + max_chunk).min(samples.len());

        let frame_search_start = search_start / hop;
        let frame_search_end = search_end / hop;

        let mut best_frame = None;
        let mut best_silence_db = f64::INFINITY;
        let target_frame = (current_pos + target_samples) / hop;

        for (frame_idx, &is_silent) in silence_mask
            .iter()
            .enumerate()
            .take(frame_search_end.min(silence_mask.len()))
            .skip(frame_search_start)
        {
            if is_silent {
                let dist = (frame_idx as i64 - target_frame as i64).abs() as f64;
                if dist < best_silence_db {
                    best_silence_db = dist;
                    best_frame = Some(frame_idx);
                }
            }
        }

        let split_sample = if let Some(frame) = best_frame {
            frame * hop
        } else {
            current_pos + target_samples
        };

        split_points.push(split_sample);
        current_pos = split_sample;
    }

    split_points
}

pub fn split_audio_at_points<'a>(samples: &'a [f32], split_points: &'a [usize]) -> Vec<&'a [f32]> {
    if split_points.is_empty() {
        return vec![samples];
    }

    let mut chunks = Vec::with_capacity(split_points.len() + 1);
    let mut prev = 0;
    for &point in split_points {
        if point > prev && point <= samples.len() {
            chunks.push(&samples[prev..point]);
            prev = point;
        }
    }
    if prev < samples.len() {
        chunks.push(&samples[prev..]);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_rms_energy_silence() {
        let samples = vec![0.0f32; 16000];
        let rms = compute_rms_energy(&samples, 3200, 1600);
        for &v in &rms {
            assert!(v.abs() < 1e-10);
        }
    }

    #[test]
    fn test_compute_rms_energy_signal() {
        let samples = vec![1.0f32; 16000];
        let rms = compute_rms_energy(&samples, 3200, 1600);
        for &v in &rms {
            assert!((v - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_rms_to_db() {
        let rms = vec![1.0, 0.5, 0.1];
        let db = rms_to_db(&rms);
        assert!((db[0]).abs() < 1e-6);
        assert!(db[1] < 0.0);
        assert!(db[2] < db[1]);
    }

    #[test]
    fn test_detect_silence() {
        let db = vec![-10.0, -50.0, -30.0, -45.0];
        let silence = detect_silence(&db, -40.0);
        assert_eq!(silence, vec![false, true, false, true]);
    }

    #[test]
    fn test_short_audio_no_chunking() {
        let samples = vec![0.5f32; 16000 * 10];
        let split_points = find_split_points(&samples, 30.0);
        assert!(split_points.is_empty());
    }

    #[test]
    fn test_split_audio_at_points() {
        let samples: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let chunks = split_audio_at_points(&samples, &[30, 60]);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 30);
        assert_eq!(chunks[1].len(), 30);
        assert_eq!(chunks[2].len(), 40);
    }

    #[test]
    fn test_split_audio_no_points() {
        let samples = vec![1.0f32; 50];
        let chunks = split_audio_at_points(&samples, &[]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 50);
    }
}
