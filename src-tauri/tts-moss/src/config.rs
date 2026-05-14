const DEFAULT_MODEL_DIR: &str = "../models/moss-tts/MOSS-TTS-Nano-100M-ONNX";
const STANDARD_TTS_PACKAGE_DIR: &str = "tts/moss-tts-nano-100m-onnx";
const MOSS_TTS_COMPONENT_DIR: &str = "MOSS-TTS-Nano-100M-ONNX";
const MANIFEST_FILE: &str = "browser_poc_manifest.json";
const DEFAULT_VOICE: &str = "Junhao";

#[derive(Debug, Clone)]
pub struct MossModelConfig {
    pub model_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, Default)]
struct MossGenerationConfig {
    seed: Option<u64>,
    max_new_frames: Option<usize>,
}

impl MossGenerationConfig {
    fn from_tts_config(config: &TtsConfig) -> Self {
        let moss = config.moss.as_ref();
        Self {
            seed: moss.and_then(|moss| moss.seed),
            max_new_frames: moss.and_then(|moss| moss.max_new_frames),
        }
    }

    fn frame_limit(self, assets: &MossAssets) -> usize {
        self.max_new_frames.unwrap_or_else(|| assets.max_new_frames())
    }
}

impl MossModelConfig {
    pub fn from_env() -> Self {
        let model_dir = std::env::var("MOSS_TTS_MODEL_DIR")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("VOICE_CODING_MODEL_HOME").map(|home| {
                    PathBuf::from(home)
                        .join(STANDARD_TTS_PACKAGE_DIR)
                        .join(MOSS_TTS_COMPONENT_DIR)
                })
            })
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MODEL_DIR));
        Self { model_dir }
    }
}
