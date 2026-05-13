#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MossSamplingMode {
    Fixed,
}

impl MossSamplingMode {
    fn from_config(config: &TtsConfig) -> Result<Self, MossTtsError> {
        let Some(mode) = config
            .moss
            .as_ref()
            .and_then(|moss| moss.sampling_mode.as_deref())
            .map(str::trim)
            .filter(|mode| !mode.is_empty())
        else {
            return Ok(Self::Fixed);
        };

        match mode.to_ascii_lowercase().as_str() {
            "fixed" => Ok(Self::Fixed),
            _ => Err(MossTtsError::UnknownSamplingMode {
                mode: mode.to_string(),
            }),
        }
    }
}
