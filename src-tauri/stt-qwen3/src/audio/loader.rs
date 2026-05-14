use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use stt_core::SttError;

const TARGET_SAMPLE_RATE: u32 = 16000;
const MIN_DURATION_SEC: f64 = 0.1;

pub fn load_audio_from_file(path: &str) -> Result<Vec<f32>, SttError> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(SttError::AudioLoadError(format!(
            "File not found: {}",
            path
        )));
    }

    let mut hint = Hint::new();
    if let Some(ext) = path_obj.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let file = std::fs::File::open(path)
        .map_err(|e| SttError::AudioLoadError(format!("Cannot open file {}: {}", path, e)))?;

    let mss = symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());

    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| SttError::AudioLoadError(format!("Failed to probe audio format: {:?}", e)))?;

    let mut reader = probed.format;

    let track = reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| SttError::AudioLoadError("No supported audio track found".into()))?;

    let track_id = track.id;
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(TARGET_SAMPLE_RATE);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1usize);

    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &decoder_opts)
        .map_err(|e| SttError::AudioLoadError(format!("Failed to create decoder: {:?}", e)))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match reader.next_packet() {
            Ok(packet) => {
                if packet.track_id() != track_id {
                    continue;
                }
                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        append_samples(&decoded, channels, &mut all_samples);
                    }
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(e) => {
                        return Err(SttError::AudioLoadError(format!("Decode error: {:?}", e)));
                    }
                }
            }
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(SttError::AudioLoadError(format!("Read error: {:?}", e)));
            }
        }
    }

    if sample_rate != TARGET_SAMPLE_RATE {
        all_samples = resample(&all_samples, sample_rate, TARGET_SAMPLE_RATE)?;
    }

    validate_samples(&all_samples, TARGET_SAMPLE_RATE)?;

    Ok(all_samples)
}

pub fn load_audio_from_bytes(data: &[u8]) -> Result<Vec<f32>, SttError> {
    let cursor = Cursor::new(data.to_vec());
    let mss = symphonia::core::io::MediaSourceStream::new(Box::new(cursor), Default::default());

    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&Hint::new(), mss, &format_opts, &metadata_opts)
        .map_err(|e| {
            SttError::AudioLoadError(format!("Failed to probe audio from bytes: {:?}", e))
        })?;

    let mut reader = probed.format;

    let track = reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| {
            SttError::AudioLoadError("No supported audio track found in bytes".into())
        })?;

    let track_id = track.id;
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(TARGET_SAMPLE_RATE);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1usize);

    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &decoder_opts)
        .map_err(|e| SttError::AudioLoadError(format!("Failed to create decoder: {:?}", e)))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        match reader.next_packet() {
            Ok(packet) => {
                if packet.track_id() != track_id {
                    continue;
                }
                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        append_samples(&decoded, channels, &mut all_samples);
                    }
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(e) => {
                        return Err(SttError::AudioLoadError(format!("Decode error: {:?}", e)));
                    }
                }
            }
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(SttError::AudioLoadError(format!("Read error: {:?}", e)));
            }
        }
    }

    if sample_rate != TARGET_SAMPLE_RATE {
        all_samples = resample(&all_samples, sample_rate, TARGET_SAMPLE_RATE)?;
    }

    validate_samples(&all_samples, TARGET_SAMPLE_RATE)?;

    Ok(all_samples)
}

fn append_samples(decoded: &AudioBufferRef, channels_hint: usize, output: &mut Vec<f32>) {
    let channels = decoded.spec().channels.count().max(channels_hint).max(1);
    let mut sample_buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
    sample_buffer.copy_interleaved_ref(decoded.clone());
    let samples = sample_buffer.samples();
    if channels > 1 {
        downmix_interleaved(samples, channels, output);
    } else {
        output.extend_from_slice(samples);
    }
}

fn downmix_interleaved(samples: &[f32], channels: usize, output: &mut Vec<f32>) {
    for frame in samples.chunks(channels) {
        let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
        output.push(mono);
    }
}

pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, SttError> {
    if from_rate == 0 || to_rate == 0 {
        return Err(SttError::AudioLoadError(format!(
            "sample rates must be greater than zero, got {from_rate} -> {to_rate}"
        )));
    }

    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        if src_idx + 1 < samples.len() {
            let s0 = samples[src_idx];
            let s1 = samples[src_idx + 1];
            output.push((s0 as f64 * (1.0 - frac) + s1 as f64 * frac) as f32);
        } else if src_idx < samples.len() {
            output.push(samples[src_idx]);
        }
    }

    Ok(output)
}

pub fn validate_samples(samples: &[f32], sample_rate: u32) -> Result<(), SttError> {
    if sample_rate == 0 {
        return Err(SttError::AudioLoadError(
            "sample rate must be greater than zero".into(),
        ));
    }

    if samples.is_empty() {
        return Err(SttError::AudioLoadError("Audio contains no samples".into()));
    }

    let duration = samples.len() as f64 / sample_rate as f64;
    if duration < MIN_DURATION_SEC {
        return Err(SttError::AudioLoadError(format!(
            "Audio duration ({:.3}s) is less than minimum ({:.1}s)",
            duration, MIN_DURATION_SEC
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_samples_too_short() {
        let samples = vec![0.5f32; 100];
        let result = validate_samples(&samples, 16000);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("minimum"));
    }

    #[test]
    fn test_validate_samples_ok() {
        let samples = vec![0.5f32; 16000];
        let result = validate_samples(&samples, 16000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_samples_empty() {
        let samples: Vec<f32> = vec![];
        let result = validate_samples(&samples, 16000);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_audio_missing_file() {
        let result = load_audio_from_file("/nonexistent/file.wav");
        assert!(result.is_err());
        match result.unwrap_err() {
            SttError::AudioLoadError(msg) => assert!(msg.contains("not found")),
            other => panic!("Expected AudioLoadError, got {:?}", other),
        }
    }

    #[test]
    fn test_load_audio_invalid_bytes() {
        let result = load_audio_from_bytes(&[0u8; 100]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resample_identity() {
        let samples = vec![0.5f32; 16000];
        let result = resample(&samples, 16000, 16000).unwrap();
        assert_eq!(result.len(), 16000);
    }

    #[test]
    fn test_resample_downsample() {
        let samples = vec![1.0f32; 48000];
        let result = resample(&samples, 48000, 16000).unwrap();
        assert_eq!(result.len(), 16000);
    }

    #[test]
    fn test_downmix_interleaved() {
        let samples = vec![1.0f32, 3.0f32, 2.0f32, 4.0f32];
        let mut output = Vec::new();
        downmix_interleaved(&samples, 2, &mut output);
        assert_eq!(output.len(), 2);
        assert!((output[0] - 2.0f32).abs() < 1e-6);
        assert!((output[1] - 3.0f32).abs() < 1e-6);
    }

    #[test]
    fn test_validate_samples_rejects_zero_sample_rate() {
        let samples = vec![0.5f32; 16000];
        let result = validate_samples(&samples, 0);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("sample rate"));
    }

    #[test]
    fn test_resample_rejects_zero_sample_rate() {
        let result = resample(&[0.5f32; 16], 0, 16000);
        assert!(result.is_err());
    }
}
