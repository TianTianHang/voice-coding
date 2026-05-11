#[derive(Debug, Default)]
struct PcmChunkBuffer {
    chunks: Vec<Vec<f32>>,
}

impl PcmChunkBuffer {
    fn push_chunk(&mut self, samples: Vec<f32>) {
        if !samples.is_empty() {
            self.chunks.push(samples);
        }
    }

    fn into_tts_result(self) -> Result<TtsResult, MossTtsError> {
        let total_samples = self
            .chunks
            .iter()
            .try_fold(0usize, |total, chunk| total.checked_add(chunk.len()))
            .ok_or_else(|| {
                MossTtsError::OutputFormat("streaming decode PCM length overflowed usize".to_string())
            })?;
        let mut pcm = Vec::with_capacity(total_samples);
        for mut chunk in self.chunks {
            pcm.append(&mut chunk);
        }
        if pcm.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "codec_decode_step",
                detail: "streaming decode produced no PCM chunks".to_string(),
            });
        }
        if pcm.len() % PLAYBACK_CHANNELS as usize != 0 {
            return Err(MossTtsError::OutputFormat(
                "streaming decode PCM length is not aligned to stereo channels".to_string(),
            ));
        }
        Ok(TtsResult {
            audio: AudioBuffer {
                sample_rate_hz: PLAYBACK_SAMPLE_RATE_HZ,
                channels: PLAYBACK_CHANNELS,
                pcm: PcmData::F32(pcm),
            },
        })
    }
}

fn codec_decode_step_unavailable(detail: String) -> MossTtsError {
    MossTtsError::Inference {
        stage: "codec_decode_step",
        detail,
    }
}

#[derive(Debug, Clone)]
struct FrameBudget {
    max_batch_size: usize,
    target_chunk_samples: Option<usize>,
    produced_pcm_samples: usize,
    decoded_frames: usize,
}

impl FrameBudget {
    fn new(metadata_batch_size: usize, requested_chunk_ms: Option<u32>) -> Self {
        let target_chunk_samples = requested_chunk_ms
            .filter(|ms| *ms > 0)
            .map(|ms| {
                (PLAYBACK_SAMPLE_RATE_HZ as usize * ms as usize / 1_000)
                    * PLAYBACK_CHANNELS as usize
            });
        Self {
            max_batch_size: metadata_batch_size.max(1),
            target_chunk_samples,
            produced_pcm_samples: 0,
            decoded_frames: 0,
        }
    }

    fn next_batch_size(&self, flushing: bool) -> usize {
        if flushing {
            return self.max_batch_size;
        }
        if self.decoded_frames == 0 {
            return 1.min(self.max_batch_size);
        }

        let target = self.target_chunk_samples.unwrap_or_else(|| {
            (PLAYBACK_SAMPLE_RATE_HZ as usize * PLAYBACK_CHANNELS as usize) / 5
        });
        let doubled = target.saturating_mul(2);
        let quadrupled = target.saturating_mul(4);

        if self.produced_pcm_samples < target {
            1
        } else if self.produced_pcm_samples < doubled {
            2.min(self.max_batch_size)
        } else if self.produced_pcm_samples < quadrupled {
            4.min(self.max_batch_size)
        } else {
            8.min(self.max_batch_size)
        }
        .max(1)
    }

    fn record_pcm_samples(&mut self, samples: usize) {
        self.produced_pcm_samples = self.produced_pcm_samples.saturating_add(samples);
        self.decoded_frames = self.decoded_frames.saturating_add(1);
    }
}
