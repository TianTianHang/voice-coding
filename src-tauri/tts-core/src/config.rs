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
    pub min_text_chunk_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flush_on_punctuation: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_buffered_text_chars: Option<usize>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MossTtsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_audio_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tts_config_defaults_to_no_stream_config() {
        assert!(TtsConfig::default().stream.is_none());
    }
}
