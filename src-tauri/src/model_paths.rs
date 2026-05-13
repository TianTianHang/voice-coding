use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::Manager;

pub const QWEN3_ASR_MODEL_ID: &str = "qwen3-asr-0.6b-onnx";
pub const QWEN3_ASR_ENGINE_NAME: &str = "qwen3-asr-0.6b";
pub const MOSS_TTS_MODEL_ID: &str = "moss-tts-nano-100m-onnx";
pub const MOSS_TTS_ENGINE_NAME: &str = "moss-onnx-tts";
pub const MOSS_TTS_COMPONENT_DIR: &str = "MOSS-TTS-Nano-100M-ONNX";
pub const MOSS_CODEC_COMPONENT_DIR: &str = "MOSS-Audio-Tokenizer-Nano-ONNX";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Asr,
    Tts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelPathSource {
    EngineEnv,
    ModelHomeEnv,
    AppData,
    DevFallback,
    LegacyDevFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingModelFile {
    pub path: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPathSnapshot {
    pub kind: ModelKind,
    pub model_id: String,
    pub engine_name: String,
    pub package_dir: String,
    pub model_dir: String,
    pub source: ModelPathSource,
    pub legacy_layout: bool,
    pub missing_files: Vec<MissingModelFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelPath {
    pub kind: ModelKind,
    pub model_id: &'static str,
    pub engine_name: &'static str,
    pub package_dir: PathBuf,
    pub engine_model_dir: PathBuf,
    pub source: ModelPathSource,
    pub legacy_layout: bool,
    pub missing_files: Vec<MissingModelFile>,
    pub error: Option<String>,
}

impl ResolvedModelPath {
    pub fn engine_model_dir_string(&self) -> String {
        self.engine_model_dir.to_string_lossy().to_string()
    }

    pub fn snapshot(&self) -> ModelPathSnapshot {
        ModelPathSnapshot {
            kind: self.kind,
            model_id: self.model_id.to_string(),
            engine_name: self.engine_name.to_string(),
            package_dir: self.package_dir.to_string_lossy().to_string(),
            model_dir: self.engine_model_dir_string(),
            source: self.source,
            legacy_layout: self.legacy_layout,
            missing_files: self.missing_files.clone(),
            error: self.error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelPathContext {
    env: HashMap<String, String>,
    app_data_dir: Option<PathBuf>,
    dev_root: PathBuf,
}

impl ModelPathContext {
    pub fn from_env() -> Self {
        Self {
            env: std::env::vars().collect(),
            app_data_dir: None,
            dev_root: infer_dev_root(),
        }
    }

    pub fn from_env_with_app(app: &tauri::AppHandle) -> Self {
        let mut context = Self::from_env();
        context.app_data_dir = app.path().app_data_dir().ok();
        context
    }

    #[cfg(test)]
    fn for_test(dev_root: PathBuf) -> Self {
        Self {
            env: HashMap::new(),
            app_data_dir: None,
            dev_root,
        }
    }

    #[cfg(test)]
    fn with_env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.env.insert(key.to_string(), value.into());
        self
    }

    #[cfg(test)]
    fn with_app_data_dir(mut self, path: PathBuf) -> Self {
        self.app_data_dir = Some(path);
        self
    }

    fn env_path(&self, key: &str) -> Option<PathBuf> {
        self.env
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    fn dev_models_dir(&self) -> PathBuf {
        normalize_dev_root(&self.dev_root).join("models")
    }
}

fn infer_dev_root() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    normalize_dev_root(&current_dir)
}

fn normalize_dev_root(dev_root: &Path) -> PathBuf {
    if dev_root.file_name().is_some_and(|name| name == "src-tauri") {
        return dev_root.parent().unwrap_or(dev_root).to_path_buf();
    }

    dev_root.to_path_buf()
}

pub fn resolve_asr_model_path() -> ResolvedModelPath {
    resolve_asr_model_path_with_context(&ModelPathContext::from_env())
}

pub fn resolve_asr_model_path_with_app(app: &tauri::AppHandle) -> ResolvedModelPath {
    resolve_asr_model_path_with_context(&ModelPathContext::from_env_with_app(app))
}

pub fn resolve_tts_model_path() -> ResolvedModelPath {
    resolve_tts_model_path_with_context(&ModelPathContext::from_env())
}

pub fn resolve_tts_model_path_with_app(app: &tauri::AppHandle) -> ResolvedModelPath {
    resolve_tts_model_path_with_context(&ModelPathContext::from_env_with_app(app))
}

pub fn resolve_asr_model_path_with_context(context: &ModelPathContext) -> ResolvedModelPath {
    if let Some(model_dir) = context.env_path("STT_MODEL_DIR") {
        return finalize_asr(model_dir, ModelPathSource::EngineEnv, false);
    }

    if let Some(home) = context.env_path("VOICE_CODING_MODEL_HOME") {
        return finalize_asr(
            standard_asr_dir(&home),
            ModelPathSource::ModelHomeEnv,
            false,
        );
    }

    if let Some(app_data_dir) = &context.app_data_dir {
        let package_dir = standard_asr_dir(&app_data_dir.join("models"));
        if has_any_asr_asset(&package_dir) {
            return finalize_asr(package_dir, ModelPathSource::AppData, false);
        }
    }

    let dev_models = context.dev_models_dir();
    let standard = standard_asr_dir(&dev_models);
    if has_any_asr_asset(&standard) {
        return finalize_asr(standard, ModelPathSource::DevFallback, false);
    }
    if has_any_asr_asset(&dev_models) {
        return finalize_asr(dev_models, ModelPathSource::LegacyDevFallback, true);
    }
    finalize_asr(standard, ModelPathSource::DevFallback, false)
}

pub fn resolve_tts_model_path_with_context(context: &ModelPathContext) -> ResolvedModelPath {
    if let Some(model_dir) = context.env_path("MOSS_TTS_MODEL_DIR") {
        let package_dir = model_dir
            .parent()
            .unwrap_or(model_dir.as_path())
            .to_path_buf();
        return finalize_tts(package_dir, model_dir, ModelPathSource::EngineEnv, false);
    }

    if let Some(home) = context.env_path("VOICE_CODING_MODEL_HOME") {
        let package_dir = standard_tts_package_dir(&home);
        return finalize_tts(
            package_dir.clone(),
            package_dir.join(MOSS_TTS_COMPONENT_DIR),
            ModelPathSource::ModelHomeEnv,
            false,
        );
    }

    if let Some(app_data_dir) = &context.app_data_dir {
        let package_dir = standard_tts_package_dir(&app_data_dir.join("models"));
        let engine_dir = package_dir.join(MOSS_TTS_COMPONENT_DIR);
        if has_any_tts_asset(&engine_dir) {
            return finalize_tts(package_dir, engine_dir, ModelPathSource::AppData, false);
        }
    }

    let dev_models = context.dev_models_dir();
    let standard_package = standard_tts_package_dir(&dev_models);
    let standard_engine = standard_package.join(MOSS_TTS_COMPONENT_DIR);
    if has_any_tts_asset(&standard_engine) {
        return finalize_tts(
            standard_package,
            standard_engine,
            ModelPathSource::DevFallback,
            false,
        );
    }

    let legacy_package = dev_models.join("moss-tts");
    let legacy_engine = legacy_package.join(MOSS_TTS_COMPONENT_DIR);
    if has_any_tts_asset(&legacy_engine) {
        return finalize_tts(
            legacy_package,
            legacy_engine,
            ModelPathSource::LegacyDevFallback,
            true,
        );
    }

    finalize_tts(
        standard_package,
        standard_engine,
        ModelPathSource::DevFallback,
        false,
    )
}

fn standard_asr_dir(model_home: &Path) -> PathBuf {
    model_home.join("asr").join(QWEN3_ASR_MODEL_ID)
}

fn standard_tts_package_dir(model_home: &Path) -> PathBuf {
    model_home.join("tts").join(MOSS_TTS_MODEL_ID)
}

fn has_any_asr_asset(path: &Path) -> bool {
    path.join("tokenizer.json").exists() || path.join("onnx_models").exists()
}

fn has_any_tts_asset(path: &Path) -> bool {
    path.join("browser_poc_manifest.json").exists()
}

fn finalize_asr(
    package_dir: PathBuf,
    source: ModelPathSource,
    legacy_layout: bool,
) -> ResolvedModelPath {
    let missing_files = missing_asr_files(&package_dir);
    let error = missing_error("Qwen3 ASR", &package_dir, &missing_files);
    ResolvedModelPath {
        kind: ModelKind::Asr,
        model_id: QWEN3_ASR_MODEL_ID,
        engine_name: QWEN3_ASR_ENGINE_NAME,
        package_dir: package_dir.clone(),
        engine_model_dir: package_dir,
        source,
        legacy_layout,
        missing_files,
        error,
    }
}

fn finalize_tts(
    package_dir: PathBuf,
    engine_model_dir: PathBuf,
    source: ModelPathSource,
    legacy_layout: bool,
) -> ResolvedModelPath {
    let missing_files = missing_tts_files(&package_dir, &engine_model_dir);
    let error = missing_error("MOSS TTS", &package_dir, &missing_files);
    ResolvedModelPath {
        kind: ModelKind::Tts,
        model_id: MOSS_TTS_MODEL_ID,
        engine_name: MOSS_TTS_ENGINE_NAME,
        package_dir,
        engine_model_dir,
        source,
        legacy_layout,
        missing_files,
        error,
    }
}

fn missing_error(
    model_name: &str,
    package_dir: &Path,
    missing_files: &[MissingModelFile],
) -> Option<String> {
    if missing_files.is_empty() {
        None
    } else {
        Some(format!(
            "{model_name} model assets are incomplete at {}: missing {}",
            package_dir.display(),
            missing_files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

fn missing_asr_files(model_dir: &Path) -> Vec<MissingModelFile> {
    let mut missing = Vec::new();
    require_file(&mut missing, model_dir, "tokenizer.json", "tokenizer");
    require_file(&mut missing, model_dir, "config.json", "model config");
    require_file(
        &mut missing,
        model_dir,
        "embed_tokens.bin",
        "token embeddings",
    );
    require_any_file(
        &mut missing,
        model_dir,
        &["onnx_models/encoder.int4.onnx", "onnx_models/encoder.onnx"],
        "encoder ONNX model",
    );
    require_any_file(
        &mut missing,
        model_dir,
        &[
            "onnx_models/decoder_init.int4.onnx",
            "onnx_models/decoder_init.onnx",
        ],
        "decoder init ONNX model",
    );
    require_any_file(
        &mut missing,
        model_dir,
        &[
            "onnx_models/decoder_step.int4.onnx",
            "onnx_models/decoder_step.onnx",
        ],
        "decoder step ONNX model",
    );
    require_file(
        &mut missing,
        model_dir,
        "onnx_models/decoder_weights.int4.data",
        "decoder weights external data",
    );
    missing
}

fn missing_tts_files(package_dir: &Path, engine_model_dir: &Path) -> Vec<MissingModelFile> {
    let mut missing = Vec::new();
    require_file_relative_to(
        &mut missing,
        package_dir,
        engine_model_dir,
        "browser_poc_manifest.json",
        "TTS manifest",
    );
    require_file_relative_to(
        &mut missing,
        package_dir,
        engine_model_dir,
        "tts_browser_onnx_meta.json",
        "TTS metadata",
    );
    require_file_relative_to(
        &mut missing,
        package_dir,
        engine_model_dir,
        "tokenizer.model",
        "TTS tokenizer",
    );
    for file in [
        "moss_tts_prefill.onnx",
        "moss_tts_decode_step.onnx",
        "moss_tts_global_shared.data",
        "moss_tts_local_fixed_sampled_frame.onnx",
        "moss_tts_local_shared.data",
    ] {
        require_file_relative_to(
            &mut missing,
            package_dir,
            engine_model_dir,
            file,
            "TTS asset",
        );
    }
    let codec_dir = package_dir.join(MOSS_CODEC_COMPONENT_DIR);
    for file in [
        "codec_browser_onnx_meta.json",
        "moss_audio_tokenizer_encode.onnx",
        "moss_audio_tokenizer_encode.data",
        "moss_audio_tokenizer_decode_step.onnx",
        "moss_audio_tokenizer_decode_shared.data",
    ] {
        require_file_relative_to(&mut missing, package_dir, &codec_dir, file, "codec asset");
    }
    missing
}

fn require_file(
    missing: &mut Vec<MissingModelFile>,
    base_dir: &Path,
    relative_path: &str,
    description: &str,
) {
    if !base_dir.join(relative_path).is_file() {
        missing.push(MissingModelFile {
            path: relative_path.to_string(),
            description: description.to_string(),
        });
    }
}

fn require_file_relative_to(
    missing: &mut Vec<MissingModelFile>,
    package_dir: &Path,
    base_dir: &Path,
    relative_path: &str,
    description: &str,
) {
    if base_dir.join(relative_path).is_file() {
        return;
    }
    let display_path = base_dir
        .join(relative_path)
        .strip_prefix(package_dir)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| relative_path.to_string());
    missing.push(MissingModelFile {
        path: display_path,
        description: description.to_string(),
    });
}

fn require_any_file(
    missing: &mut Vec<MissingModelFile>,
    base_dir: &Path,
    candidates: &[&str],
    description: &str,
) {
    if candidates
        .iter()
        .any(|candidate| base_dir.join(candidate).is_file())
    {
        return;
    }
    missing.push(MissingModelFile {
        path: candidates.join(" or "),
        description: description.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn asr_engine_env_takes_priority() {
        let fixture = Fixture::new();
        fixture.write_complete_asr(&fixture.engine_asr_dir());
        fixture.write_complete_asr(&fixture.standard_asr_dir());
        let context = fixture
            .context()
            .with_env(
                "STT_MODEL_DIR",
                fixture.engine_asr_dir().display().to_string(),
            )
            .with_env(
                "VOICE_CODING_MODEL_HOME",
                fixture.home.display().to_string(),
            );

        let resolved = resolve_asr_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::EngineEnv);
        assert_eq!(resolved.engine_model_dir, fixture.engine_asr_dir());
        assert!(!resolved.legacy_layout);
        assert!(resolved.missing_files.is_empty());
    }

    #[test]
    fn asr_uses_model_home_standard_layout() {
        let fixture = Fixture::new();
        fixture.write_complete_asr(&fixture.standard_asr_dir());
        let context = fixture.context().with_env(
            "VOICE_CODING_MODEL_HOME",
            fixture.home.display().to_string(),
        );

        let resolved = resolve_asr_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::ModelHomeEnv);
        assert_eq!(resolved.engine_model_dir, fixture.standard_asr_dir());
        assert!(resolved.missing_files.is_empty());
    }

    #[test]
    fn asr_uses_app_data_before_dev_fallback_when_assets_exist() {
        let fixture = Fixture::new();
        let app_home = fixture.temp.path().join("app-data").join("models");
        let app_asr = standard_asr_dir(&app_home);
        fixture.write_complete_asr(&app_asr);
        fixture.write_complete_asr(&fixture.standard_asr_dir());
        let context = fixture
            .context()
            .with_app_data_dir(fixture.temp.path().join("app-data"));

        let resolved = resolve_asr_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::AppData);
        assert_eq!(resolved.engine_model_dir, app_asr);
    }

    #[test]
    fn asr_detects_legacy_dev_layout() {
        let fixture = Fixture::new();
        fixture.write_complete_asr(&fixture.dev_models_dir());

        let resolved = resolve_asr_model_path_with_context(&fixture.context());

        assert_eq!(resolved.source, ModelPathSource::LegacyDevFallback);
        assert_eq!(resolved.engine_model_dir, fixture.dev_models_dir());
        assert!(resolved.legacy_layout);
    }

    #[test]
    fn asr_reports_missing_files() {
        let fixture = Fixture::new();
        std::fs::create_dir_all(fixture.standard_asr_dir()).unwrap();
        std::fs::write(fixture.standard_asr_dir().join("tokenizer.json"), b"{}").unwrap();

        let resolved = resolve_asr_model_path_with_context(&fixture.context());

        assert!(resolved.error.as_ref().unwrap().contains("Qwen3 ASR"));
        assert!(resolved
            .missing_files
            .iter()
            .any(|file| file.path == "config.json"));
        assert!(resolved
            .missing_files
            .iter()
            .any(|file| file.path.contains("encoder")));
    }

    #[test]
    fn tts_engine_env_points_to_component_dir() {
        let fixture = Fixture::new();
        let package = fixture.temp.path().join("explicit-moss");
        let engine = package.join(MOSS_TTS_COMPONENT_DIR);
        fixture.write_complete_tts(&package);
        let context = fixture
            .context()
            .with_env("MOSS_TTS_MODEL_DIR", engine.display().to_string())
            .with_env(
                "VOICE_CODING_MODEL_HOME",
                fixture.home.display().to_string(),
            );

        let resolved = resolve_tts_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::EngineEnv);
        assert_eq!(resolved.package_dir, package);
        assert_eq!(resolved.engine_model_dir, engine);
        assert!(!resolved.legacy_layout);
        assert!(resolved.missing_files.is_empty());
    }

    #[test]
    fn tts_uses_model_home_standard_layout() {
        let fixture = Fixture::new();
        fixture.write_complete_tts(&fixture.standard_tts_package_dir());
        let context = fixture.context().with_env(
            "VOICE_CODING_MODEL_HOME",
            fixture.home.display().to_string(),
        );

        let resolved = resolve_tts_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::ModelHomeEnv);
        assert_eq!(resolved.package_dir, fixture.standard_tts_package_dir());
        assert_eq!(resolved.engine_model_dir, fixture.standard_tts_engine_dir());
    }

    #[test]
    fn tts_dev_fallback_uses_project_root_when_running_from_src_tauri() {
        let fixture = Fixture::new();
        fixture.write_complete_tts(&fixture.project_standard_tts_package_dir());
        let context = ModelPathContext::for_test(fixture.dev_root.join("src-tauri"));

        let resolved = resolve_tts_model_path_with_context(&context);

        assert_eq!(resolved.source, ModelPathSource::DevFallback);
        assert_eq!(
            resolved.package_dir,
            fixture.project_standard_tts_package_dir()
        );
        assert_eq!(
            resolved.engine_model_dir,
            fixture.project_standard_tts_engine_dir()
        );
        assert!(resolved.missing_files.is_empty());
    }

    #[test]
    fn tts_detects_legacy_dev_layout() {
        let fixture = Fixture::new();
        let legacy_package = fixture.dev_models_dir().join("moss-tts");
        fixture.write_complete_tts(&legacy_package);

        let resolved = resolve_tts_model_path_with_context(&fixture.context());

        assert_eq!(resolved.source, ModelPathSource::LegacyDevFallback);
        assert_eq!(resolved.package_dir, legacy_package);
        assert!(resolved.legacy_layout);
    }

    #[test]
    fn tts_reports_missing_codec_files() {
        let fixture = Fixture::new();
        let package = fixture.standard_tts_package_dir();
        std::fs::create_dir_all(package.join(MOSS_TTS_COMPONENT_DIR)).unwrap();
        std::fs::write(
            package
                .join(MOSS_TTS_COMPONENT_DIR)
                .join("browser_poc_manifest.json"),
            b"{}",
        )
        .unwrap();

        let resolved = resolve_tts_model_path_with_context(&fixture.context());

        assert!(resolved.error.as_ref().unwrap().contains("MOSS TTS"));
        assert!(resolved
            .missing_files
            .iter()
            .any(|file| file.path.contains(MOSS_CODEC_COMPONENT_DIR)));
    }

    struct Fixture {
        temp: TempDir,
        home: PathBuf,
        dev_root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let home = temp.path().join("home");
            let dev_root = temp.path().join("repo");
            Self {
                temp,
                home,
                dev_root,
            }
        }

        fn context(&self) -> ModelPathContext {
            ModelPathContext::for_test(self.dev_root.clone())
        }

        fn dev_models_dir(&self) -> PathBuf {
            self.dev_root.join("models")
        }

        fn standard_asr_dir(&self) -> PathBuf {
            standard_asr_dir(&self.home)
        }

        fn engine_asr_dir(&self) -> PathBuf {
            self.temp.path().join("explicit-asr")
        }

        fn standard_tts_package_dir(&self) -> PathBuf {
            standard_tts_package_dir(&self.home)
        }

        fn standard_tts_engine_dir(&self) -> PathBuf {
            self.standard_tts_package_dir().join(MOSS_TTS_COMPONENT_DIR)
        }

        fn project_standard_tts_package_dir(&self) -> PathBuf {
            standard_tts_package_dir(&self.dev_models_dir())
        }

        fn project_standard_tts_engine_dir(&self) -> PathBuf {
            self.project_standard_tts_package_dir()
                .join(MOSS_TTS_COMPONENT_DIR)
        }

        fn write_complete_asr(&self, dir: &Path) {
            std::fs::create_dir_all(dir.join("onnx_models")).unwrap();
            for file in ["tokenizer.json", "config.json", "embed_tokens.bin"] {
                std::fs::write(dir.join(file), b"x").unwrap();
            }
            for file in [
                "encoder.int4.onnx",
                "decoder_init.int4.onnx",
                "decoder_step.int4.onnx",
                "decoder_weights.int4.data",
            ] {
                std::fs::write(dir.join("onnx_models").join(file), b"x").unwrap();
            }
        }

        fn write_complete_tts(&self, package_dir: &Path) {
            let tts_dir = package_dir.join(MOSS_TTS_COMPONENT_DIR);
            let codec_dir = package_dir.join(MOSS_CODEC_COMPONENT_DIR);
            std::fs::create_dir_all(&tts_dir).unwrap();
            std::fs::create_dir_all(&codec_dir).unwrap();
            for file in [
                "browser_poc_manifest.json",
                "tts_browser_onnx_meta.json",
                "tokenizer.model",
                "moss_tts_prefill.onnx",
                "moss_tts_decode_step.onnx",
                "moss_tts_global_shared.data",
                "moss_tts_local_fixed_sampled_frame.onnx",
                "moss_tts_local_shared.data",
            ] {
                std::fs::write(tts_dir.join(file), b"x").unwrap();
            }
            for file in [
                "codec_browser_onnx_meta.json",
                "moss_audio_tokenizer_encode.onnx",
                "moss_audio_tokenizer_encode.data",
                "moss_audio_tokenizer_decode_step.onnx",
                "moss_audio_tokenizer_decode_shared.data",
            ] {
                std::fs::write(codec_dir.join(file), b"x").unwrap();
            }
        }
    }
}
