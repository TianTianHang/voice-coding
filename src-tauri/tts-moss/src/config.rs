const DEFAULT_MODEL_DIR: &str = "../models/moss-tts/MOSS-TTS-Nano-100M-ONNX";
const MANIFEST_FILE: &str = "browser_poc_manifest.json";
const DEFAULT_VOICE: &str = "Junhao";
const USE_CODEC_DECODE_STEP_BY_DEFAULT: bool = false;

#[derive(Debug, Clone)]
pub struct MossModelConfig {
    pub model_dir: PathBuf,
}

impl MossModelConfig {
    pub fn from_env() -> Self {
        let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MODEL_DIR));
        Self { model_dir }
    }
}
