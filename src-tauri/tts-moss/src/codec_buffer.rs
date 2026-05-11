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
