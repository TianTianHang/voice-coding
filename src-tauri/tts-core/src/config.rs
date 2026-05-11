#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsConfig {
    pub voice: Option<String>,
    pub speed: Option<f32>,
    pub pitch: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moss: Option<MossTtsConfig>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MossTtsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_audio_path: Option<String>,
}
