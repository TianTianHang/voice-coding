#[derive(Debug, Clone)]
pub struct MossAssets {
    #[allow(dead_code)]
    pub manifest_path: PathBuf,
    #[allow(dead_code)]
    pub tts_meta_path: PathBuf,
    #[allow(dead_code)]
    pub codec_meta_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub tts_files: HashMap<String, PathBuf>,
    pub codec_files: HashMap<String, PathBuf>,
    manifest: Manifest,
    tts_meta: TtsMeta,
    codec_meta: CodecMeta,
}

impl MossAssets {
    pub fn load(config: MossModelConfig) -> Result<Self, MossTtsError> {
        let manifest_path = config.model_dir.join(MANIFEST_FILE);
        ensure_file(&manifest_path, "manifest")?;
        let manifest: Manifest = read_json(&manifest_path)?;

        let tts_meta_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.tts_meta",
            &manifest.model_files.tts_meta,
        )?;
        let codec_meta_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.codec_meta",
            &manifest.model_files.codec_meta,
        )?;
        let tokenizer_path = resolve_manifest_path(
            &config.model_dir,
            "model_files.tokenizer_model",
            &manifest.model_files.tokenizer_model,
        )?;
        ensure_file(&tts_meta_path, "tts meta")?;
        ensure_file(&codec_meta_path, "codec meta")?;
        ensure_file(&tokenizer_path, "tokenizer")?;

        let tts_meta: TtsMeta = read_json(&tts_meta_path)?;
        let codec_meta: CodecMeta = read_json(&codec_meta_path)?;
        validate_meta_consistency(&manifest, &tts_meta, &codec_meta)?;

        let tts_files = validate_required_model_files(
            tts_meta_path.parent().unwrap_or_else(|| Path::new("")),
            "tts",
            &tts_meta.files,
            &tts_meta.external_data_files,
            &["prefill", "decode_step", "local_fixed_sampled_frame"],
        )?;
        let codec_files = validate_required_model_files(
            codec_meta_path.parent().unwrap_or_else(|| Path::new("")),
            "codec",
            &codec_meta.files,
            &codec_meta.external_data_files,
            &["encode", "decode_step"],
        )?;

        Ok(Self {
            manifest_path,
            tts_meta_path,
            codec_meta_path,
            tokenizer_path,
            tts_files,
            codec_files,
            manifest,
            tts_meta,
            codec_meta,
        })
    }

    fn resolve_voice(&self, requested: Option<&str>) -> Result<&BuiltinVoice, MossTtsError> {
        let voice = requested
            .map(str::trim)
            .filter(|voice| !voice.is_empty())
            .unwrap_or(DEFAULT_VOICE);
        self.manifest
            .builtin_voices
            .iter()
            .find(|candidate| candidate.voice.eq_ignore_ascii_case(voice))
            .ok_or_else(|| MossTtsError::UnknownVoice {
                voice: voice.to_string(),
                available: self.available_voices_summary(),
            })
    }

    fn available_voices_summary(&self) -> String {
        self.manifest
            .builtin_voices
            .iter()
            .take(8)
            .map(|voice| voice.voice.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn n_vq(&self) -> usize {
        self.manifest.tts_config.n_vq as usize
    }

    fn row_width(&self) -> usize {
        self.n_vq() + 1
    }

    fn audio_codebook_size(&self) -> usize {
        self.manifest
            .tts_config
            .audio_codebook_sizes
            .first()
            .copied()
            .unwrap_or(1024) as usize
    }

    fn max_new_frames(&self) -> usize {
        self.manifest.generation_defaults.max_new_frames as usize
    }

    fn text_row(&self, token_id: i64) -> Vec<i64> {
        let mut row = vec![self.manifest.tts_config.audio_pad_token_id; self.row_width()];
        row[0] = token_id;
        row
    }

    fn audio_row(&self, codes: &[i64], slot_token_id: i64) -> Vec<i64> {
        let mut row = vec![self.manifest.tts_config.audio_pad_token_id; self.row_width()];
        row[0] = slot_token_id;
        for (index, code) in codes.iter().take(self.n_vq()).enumerate() {
            row[index + 1] = *code;
        }
        row
    }

    fn build_voice_clone_request_rows(
        &self,
        text_token_ids: impl IntoIterator<Item = i64>,
        prompt_audio_codes: &[Vec<i64>],
    ) -> Result<MossRequestRows, MossTtsError> {
        let mut rows = Vec::new();
        for token_id in self
            .manifest
            .prompt_templates
            .user_prompt_prefix_token_ids
            .iter()
            .copied()
            .chain(std::iter::once(self.manifest.tts_config.audio_start_token_id))
        {
            rows.push(self.text_row(token_id));
        }
        for codes in prompt_audio_codes {
            rows.push(self.audio_row(codes, self.manifest.tts_config.audio_user_slot_token_id));
        }
        for token_id in std::iter::once(self.manifest.tts_config.audio_end_token_id)
            .chain(
                self.manifest
                    .prompt_templates
                    .user_prompt_after_reference_token_ids
                    .iter()
                    .copied(),
            )
            .chain(text_token_ids)
            .chain(
                self.manifest
                    .prompt_templates
                    .assistant_prompt_prefix_token_ids
                    .iter()
                    .copied(),
            )
            .chain(std::iter::once(self.manifest.tts_config.audio_start_token_id))
        {
            rows.push(self.text_row(token_id));
        }
        if rows.is_empty() {
            return Err(MossTtsError::Inference {
                stage: "tts_prompt",
                detail: "MOSS prompt rows are empty".to_string(),
            });
        }
        Ok(MossRequestRows {
            attention_mask: vec![1; rows.len()],
            rows,
        })
    }
}

struct MossRequestRows {
    rows: Vec<Vec<i64>>,
    attention_mask: Vec<i64>,
}
