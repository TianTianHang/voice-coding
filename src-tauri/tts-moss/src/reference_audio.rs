use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use tts_core::{PLAYBACK_CHANNELS, PLAYBACK_SAMPLE_RATE_HZ};

use crate::MossTtsError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReferenceAudio {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
    pub channels: u16,
}

impl ReferenceAudio {
    pub(crate) fn from_wav_path(path: &Path) -> Result<Self, MossTtsError> {
        match detect_reference_audio_format(path)? {
            ReferenceAudioFormat::Wav => load_wav_reference_audio(path),
            ReferenceAudioFormat::Flac => load_symphonia_reference_audio(path, "flac"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceAudioFormat {
    Wav,
    Flac,
}

fn detect_reference_audio_format(path: &Path) -> Result<ReferenceAudioFormat, MossTtsError> {
    let mut file = File::open(path).map_err(|e| MossTtsError::Inference {
        stage: "reference_audio",
        detail: format!("failed to open {}: {e}", path.display()),
    })?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("failed to read {}: {e}", path.display()),
        })?;
    match &magic {
        b"RIFF" => Ok(ReferenceAudioFormat::Wav),
        b"fLaC" => Ok(ReferenceAudioFormat::Flac),
        _ => Err(MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!(
                "unsupported reference audio format for {}; expected WAV or FLAC",
                path.display()
            ),
        }),
    }
}

fn load_wav_reference_audio(path: &Path) -> Result<ReferenceAudio, MossTtsError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| MossTtsError::Inference {
        stage: "reference_audio",
        detail: format!("failed to open {}: {e}", path.display()),
    })?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(MossTtsError::Inference {
            stage: "reference_audio",
            detail: "reference WAV has invalid channel count or sample rate".to_string(),
        });
    }

    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| MossTtsError::Inference {
                stage: "reference_audio",
                detail: e.to_string(),
            })?,
        hound::SampleFormat::Int => match spec.bits_per_sample {
            8 => reader
                .samples::<i8>()
                .map(|sample| sample.map(|value| value as f32 / i8::MAX as f32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| MossTtsError::Inference {
                    stage: "reference_audio",
                    detail: e.to_string(),
                })?,
            16 => reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| value as f32 / 32768.0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| MossTtsError::Inference {
                    stage: "reference_audio",
                    detail: e.to_string(),
                })?,
            24 | 32 => reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / 2_147_483_648.0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| MossTtsError::Inference {
                    stage: "reference_audio",
                    detail: e.to_string(),
                })?,
            bits => {
                return Err(MossTtsError::Inference {
                    stage: "reference_audio",
                    detail: format!("unsupported WAV bit depth {bits}"),
                });
            }
        },
    };

    normalize_reference_audio(samples, spec.sample_rate, spec.channels)
}

fn load_symphonia_reference_audio(
    path: &Path,
    extension_hint: &str,
) -> Result<ReferenceAudio, MossTtsError> {
    let file = File::open(path).map_err(|e| MossTtsError::Inference {
        stage: "reference_audio",
        detail: format!("failed to open {}: {e}", path.display()),
    })?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension_hint);
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("failed to probe {}: {e:?}", path.display()),
        })?;
    let mut reader = probed.format;
    let track = reader
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("no supported audio track found in {}", path.display()),
        })?;
    let track_id = track.id;
    let codec_params = &track.codec_params;
    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("missing sample rate in {}", path.display()),
        })?;
    let channels = codec_params
        .channels
        .map(|channels| channels.count() as u16)
        .ok_or_else(|| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("missing channel layout in {}", path.display()),
        })?;
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .map_err(|e| MossTtsError::Inference {
            stage: "reference_audio",
            detail: format!("failed to create decoder for {}: {e:?}", path.display()),
        })?;
    let mut samples = Vec::new();
    loop {
        match reader.next_packet() {
            Ok(packet) => {
                if packet.track_id() != track_id {
                    continue;
                }
                match decoder.decode(&packet) {
                    Ok(decoded) => append_symphonia_samples(&decoded, &mut samples),
                    Err(SymphoniaError::DecodeError(_)) => continue,
                    Err(e) => {
                        return Err(MossTtsError::Inference {
                            stage: "reference_audio",
                            detail: format!("failed to decode {}: {e:?}", path.display()),
                        });
                    }
                }
            }
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(MossTtsError::Inference {
                    stage: "reference_audio",
                    detail: format!("failed to read {}: {e:?}", path.display()),
                });
            }
        }
    }
    normalize_reference_audio(samples, sample_rate, channels)
}

fn append_symphonia_samples(decoded: &AudioBufferRef<'_>, output: &mut Vec<f32>) {
    match decoded {
        AudioBufferRef::U8(buffer) => {
            append_planar_samples(buffer, output, |value| (value as f32 - 128.0) / 128.0)
        }
        AudioBufferRef::S16(buffer) => {
            append_planar_samples(buffer, output, |value| value as f32 / 32768.0)
        }
        AudioBufferRef::S24(buffer) => {
            append_planar_samples(buffer, output, |value| value.inner() as f32 / 8_388_608.0)
        }
        AudioBufferRef::S32(buffer) => {
            append_planar_samples(buffer, output, |value| value as f32 / 2_147_483_648.0)
        }
        AudioBufferRef::F32(buffer) => append_planar_samples(buffer, output, |value| value),
        AudioBufferRef::F64(buffer) => append_planar_samples(buffer, output, |value| value as f32),
        _ => {}
    }
}

fn append_planar_samples<T, F>(
    buffer: &symphonia::core::audio::AudioBuffer<T>,
    output: &mut Vec<f32>,
    convert: F,
) where
    T: symphonia::core::sample::Sample,
    F: Fn(T) -> f32,
{
    let channels = buffer.spec().channels.count();
    let frames = buffer.frames();
    output.reserve(frames * channels);
    for frame in 0..frames {
        for channel in 0..channels {
            output.push(convert(buffer.chan(channel)[frame]));
        }
    }
}

pub(crate) fn reference_audio_path(config: &tts_core::TtsConfig) -> Option<PathBuf> {
    config
        .moss
        .as_ref()
        .and_then(|moss| moss.reference_audio_path.as_deref())
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn normalize_reference_audio(
    samples: Vec<f32>,
    sample_rate_hz: u32,
    channels: u16,
) -> Result<ReferenceAudio, MossTtsError> {
    if samples.is_empty() {
        return Err(MossTtsError::Inference {
            stage: "reference_audio",
            detail: "reference audio contains no samples".to_string(),
        });
    }
    if channels == 0 || sample_rate_hz == 0 {
        return Err(MossTtsError::Inference {
            stage: "reference_audio",
            detail: "reference audio has invalid channel count or sample rate".to_string(),
        });
    }
    let mono = downmix_to_mono(&samples, channels)?;
    let resampled = resample_linear(&mono, sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
    let stereo = mono_to_stereo(&resampled);
    Ok(ReferenceAudio {
        samples: stereo,
        sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
        channels: PLAYBACK_CHANNELS,
    })
}

fn downmix_to_mono(samples: &[f32], channels: u16) -> Result<Vec<f32>, MossTtsError> {
    let channels = channels as usize;
    if !samples.len().is_multiple_of(channels) {
        return Err(MossTtsError::Inference {
            stage: "reference_audio",
            detail: "reference audio samples are not aligned to channels".to_string(),
        });
    }
    let mut mono = Vec::with_capacity(samples.len() / channels);
    for frame in samples.chunks_exact(channels) {
        mono.push(frame.iter().sum::<f32>() / channels as f32);
    }
    Ok(mono)
}

fn mono_to_stereo(samples: &[f32]) -> Vec<f32> {
    let mut stereo = Vec::with_capacity(samples.len() * PLAYBACK_CHANNELS as usize);
    for sample in samples {
        stereo.push(*sample);
        stereo.push(*sample);
    }
    stereo
}

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = ((samples.len() as f64) * ratio).round().max(1.0) as usize;
    let mut output = Vec::with_capacity(new_len);
    for index in 0..new_len {
        let source = index as f64 / ratio;
        let left = source.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let frac = (source - left as f64) as f32;
        output.push(samples[left] * (1.0 - frac) + samples[right] * frac);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn normalizes_mono_24khz_to_48khz_stereo() {
        let audio = normalize_reference_audio(vec![0.0, 1.0], 24_000, 1).unwrap();

        assert_eq!(audio.sample_rate_hz, PLAYBACK_SAMPLE_RATE_HZ);
        assert_eq!(audio.channels, PLAYBACK_CHANNELS);
        assert_eq!(audio.samples.len(), 8);
        assert_eq!(audio.samples[0], audio.samples[1]);
    }

    #[test]
    fn downmixes_stereo_before_resampling() {
        let audio = normalize_reference_audio(vec![1.0, -1.0, 0.5, 0.25], 48_000, 2).unwrap();

        assert_eq!(audio.samples, vec![0.0, 0.0, 0.375, 0.375]);
    }

    #[test]
    fn rejects_channel_misalignment() {
        let err = normalize_reference_audio(vec![1.0, 2.0, 3.0], 48_000, 2).unwrap_err();

        assert!(err.to_string().contains("reference_audio"));
    }

    #[test]
    fn detects_flac_by_magic_even_with_wav_extension() {
        let mut file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        file.write_all(b"fLaC").unwrap();

        let format = detect_reference_audio_format(file.path()).unwrap();

        assert_eq!(format, ReferenceAudioFormat::Flac);
    }
}
