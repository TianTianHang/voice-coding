#[derive(Debug, Default)]
struct PcmChunkBuffer {
    chunks: Vec<Vec<f32>>,
}

impl PcmChunkBuffer {
    fn from_chunk(samples: Vec<f32>) -> Self {
        let mut buffer = Self::default();
        buffer.push_chunk(samples);
        buffer
    }

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
    startup_target_frames: usize,
    adaptive_target_buffer_seconds: f64,
    total_output_seconds: f64,
    first_batch_finished_at: Option<Instant>,
    last_batch_finished_at: Option<Instant>,
    last_rtf_window: Option<f64>,
    batches_decoded: usize,
}

impl FrameBudget {
    const DEFAULT_DOWNSAMPLE_RATE: u32 = 3_840;
    const STARTUP_TARGET_SECONDS: f64 = 1.0;
    const MIN_TARGET_BUFFER_SECONDS: f64 = 0.8;
    const INITIAL_ADAPTIVE_TARGET_SECONDS: f64 = 1.0;
    const LOW_LEAD_SECONDS: f64 = 0.20;
    const MID_LEAD_SECONDS: f64 = 0.55;
    const HIGH_LEAD_SECONDS: f64 = 1.10;
    const PLENTY_LEAD_SECONDS: f64 = 1.50;

    fn new(metadata_batch_size: usize, codec_config: &CodecConfig) -> Self {
        let max_batch_size = metadata_batch_size.max(1);
        let frames_per_second = frames_per_second(codec_config);
        let startup_target_frames = (frames_per_second * Self::STARTUP_TARGET_SECONDS)
            .ceil()
            .max(1.0) as usize;
        Self {
            max_batch_size,
            startup_target_frames: startup_target_frames.min(max_batch_size).max(1),
            adaptive_target_buffer_seconds: Self::INITIAL_ADAPTIVE_TARGET_SECONDS,
            total_output_seconds: 0.0,
            first_batch_finished_at: None,
            last_batch_finished_at: None,
            last_rtf_window: None,
            batches_decoded: 0,
        }
    }

    fn next_batch_size(&self, flushing: bool) -> usize {
        if flushing {
            return self.max_batch_size;
        }
        if self.batches_decoded == 0 {
            return self.startup_target_frames;
        }

        self.clamp_batch_size(self.adaptive_batch_size())
    }

    fn record_pcm_samples(&mut self, samples: usize) {
        self.record_pcm_samples_at(samples, Instant::now());
    }

    #[cfg(test)]
    fn record_pcm_samples_after(&mut self, samples: usize, elapsed_since_last: std::time::Duration) {
        let finished_at = self
            .last_batch_finished_at
            .map(|last| last + elapsed_since_last)
            .unwrap_or_else(Instant::now);
        self.record_pcm_samples_at(samples, finished_at);
    }

    fn record_pcm_samples_at(&mut self, samples: usize, finished_at: Instant) {
        let output_seconds = pcm_output_seconds(samples);
        if output_seconds <= 0.0 {
            return;
        }

        let previous_finished_at = self.last_batch_finished_at;
        self.total_output_seconds += output_seconds;
        if self.first_batch_finished_at.is_none() {
            self.first_batch_finished_at = Some(finished_at);
        }
        if let Some(previous_finished_at) = previous_finished_at {
            let elapsed = finished_at
                .saturating_duration_since(previous_finished_at)
                .as_secs_f64();
            self.last_rtf_window = Some(elapsed / output_seconds);
        }
        self.last_batch_finished_at = Some(finished_at);
        self.batches_decoded = self.batches_decoded.saturating_add(1);
        self.adjust_target_buffer();
    }

    fn adaptive_batch_size(&self) -> usize {
        let lead = self.lead_seconds();
        if self.last_rtf_window.is_some_and(|rtf| rtf >= 1.08) || lead <= Self::LOW_LEAD_SECONDS {
            32
        } else if self.last_rtf_window.is_some_and(|rtf| rtf >= 0.98)
            || lead < Self::MID_LEAD_SECONDS
        {
            24
        } else if lead < Self::HIGH_LEAD_SECONDS {
            16
        } else if lead < self.adaptive_target_buffer_seconds {
            24
        } else {
            32
        }
    }

    fn adjust_target_buffer(&mut self) {
        let lead = self.lead_seconds();
        if self.last_rtf_window.is_some_and(|rtf| rtf >= 1.08) || lead <= Self::LOW_LEAD_SECONDS {
            self.adaptive_target_buffer_seconds =
                if self.adaptive_target_buffer_seconds < 1.5 {
                    1.5
                } else if self.adaptive_target_buffer_seconds < 2.0 {
                    2.0
                } else {
                    3.0
                };
        } else if self.last_rtf_window.is_some_and(|rtf| rtf >= 0.98)
            || lead < Self::MID_LEAD_SECONDS
        {
            self.adaptive_target_buffer_seconds =
                (self.adaptive_target_buffer_seconds + 0.25).min(2.0);
        } else if self.last_rtf_window.is_some_and(|rtf| rtf < 0.80)
            && lead >= Self::PLENTY_LEAD_SECONDS
        {
            self.adaptive_target_buffer_seconds =
                (self.adaptive_target_buffer_seconds - 0.10)
                    .max(Self::MIN_TARGET_BUFFER_SECONDS);
        }
    }

    fn lead_seconds(&self) -> f64 {
        let Some(first_batch_finished_at) = self.first_batch_finished_at else {
            return 0.0;
        };
        let Some(last_batch_finished_at) = self.last_batch_finished_at else {
            return 0.0;
        };
        self.total_output_seconds
            - last_batch_finished_at
                .saturating_duration_since(first_batch_finished_at)
                .as_secs_f64()
    }

    fn clamp_batch_size(&self, batch_size: usize) -> usize {
        batch_size.clamp(1, self.max_batch_size)
    }

    #[cfg(test)]
    fn startup_target_frames(&self) -> usize {
        self.startup_target_frames
    }

    #[cfg(test)]
    fn adaptive_target_buffer_seconds(&self) -> f64 {
        self.adaptive_target_buffer_seconds
    }

}

fn frames_per_second(codec_config: &CodecConfig) -> f64 {
    let downsample_rate = codec_config
        .downsample_rate
        .filter(|rate| *rate > 0)
        .unwrap_or(FrameBudget::DEFAULT_DOWNSAMPLE_RATE);
    (codec_config.sample_rate as f64 / downsample_rate as f64).max(1.0)
}

fn pcm_output_seconds(samples: usize) -> f64 {
    samples as f64 / PLAYBACK_CHANNELS as f64 / PLAYBACK_SAMPLE_RATE_HZ as f64
}
