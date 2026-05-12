#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsConfig {
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub pitch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<TtsStreamConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moss: Option<MossTtsConfig>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStreamConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_chunk_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_initial_buffer_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_rebuffer_threshold_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_rebuffer_target_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_decode_preview: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_text_chunk_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flush_on_punctuation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_buffered_text_chars: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackBufferConfig {
    pub initial_buffer_ms: u32,
    pub rebuffer_threshold_ms: u32,
    pub rebuffer_target_ms: u32,
}

impl Default for PlaybackBufferConfig {
    fn default() -> Self {
        Self {
            initial_buffer_ms: 600,
            rebuffer_threshold_ms: 250,
            rebuffer_target_ms: 600,
        }
    }
}

impl PlaybackBufferConfig {
    pub fn from_stream_config(config: Option<&TtsStreamConfig>) -> Self {
        let defaults = Self::default();
        let Some(config) = config else {
            return defaults;
        };
        let initial_buffer_ms = config
            .playback_initial_buffer_ms
            .unwrap_or(defaults.initial_buffer_ms);
        let rebuffer_threshold_ms = config
            .playback_rebuffer_threshold_ms
            .unwrap_or(defaults.rebuffer_threshold_ms);
        let requested_target = config
            .playback_rebuffer_target_ms
            .unwrap_or(initial_buffer_ms);
        Self {
            initial_buffer_ms,
            rebuffer_threshold_ms,
            rebuffer_target_ms: requested_target.max(rebuffer_threshold_ms),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MossTtsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_audio_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_new_frames: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_top_k: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_top_k: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_repetition_penalty: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tts_config_defaults_to_no_stream_config() {
        assert!(TtsConfig::default().stream.is_none());
    }

    #[test]
    fn playback_buffer_config_uses_debug_stream_defaults() {
        assert_eq!(
            PlaybackBufferConfig::from_stream_config(None),
            PlaybackBufferConfig {
                initial_buffer_ms: 600,
                rebuffer_threshold_ms: 250,
                rebuffer_target_ms: 600,
            }
        );
    }

    #[test]
    fn playback_buffer_config_normalizes_target_to_threshold() {
        let stream = TtsStreamConfig {
            playback_initial_buffer_ms: Some(300),
            playback_rebuffer_threshold_ms: Some(700),
            playback_rebuffer_target_ms: Some(400),
            ..TtsStreamConfig::default()
        };

        assert_eq!(
            PlaybackBufferConfig::from_stream_config(Some(&stream)),
            PlaybackBufferConfig {
                initial_buffer_ms: 300,
                rebuffer_threshold_ms: 700,
                rebuffer_target_ms: 700,
            }
        );
    }

    #[test]
    fn moss_tts_config_deserializes_private_camel_case_fields() {
        let json = r#"{
            "samplingMode": "fixed",
            "referenceAudioPath": "/tmp/ref.wav",
            "seed": 42,
            "maxNewFrames": 128,
            "textTemperature": 1.1,
            "textTopP": 0.9,
            "textTopK": 40,
            "audioTemperature": 0.7,
            "audioTopP": 0.85,
            "audioTopK": 20,
            "audioRepetitionPenalty": 1.15
        }"#;

        let config: MossTtsConfig = serde_json::from_str(json).expect("valid moss config");

        assert_eq!(config.sampling_mode.as_deref(), Some("fixed"));
        assert_eq!(config.reference_audio_path.as_deref(), Some("/tmp/ref.wav"));
        assert_eq!(config.seed, Some(42));
        assert_eq!(config.max_new_frames, Some(128));
        assert_eq!(config.text_temperature, Some(1.1));
        assert_eq!(config.text_top_p, Some(0.9));
        assert_eq!(config.text_top_k, Some(40));
        assert_eq!(config.audio_temperature, Some(0.7));
        assert_eq!(config.audio_top_p, Some(0.85));
        assert_eq!(config.audio_top_k, Some(20));
        assert_eq!(config.audio_repetition_penalty, Some(1.15));
    }
}
