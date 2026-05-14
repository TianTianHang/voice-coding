use crate::tokenizer::wrapper::TokenizerWrapper;
use stt_core::SttError;

pub trait PromptTokenizer {
    fn encode(&self, text: &str) -> Result<Vec<u32>, SttError>;
}

impl PromptTokenizer for TokenizerWrapper {
    fn encode(&self, text: &str) -> Result<Vec<u32>, SttError> {
        self.encode(text)
    }
}

pub const IM_START_ID: u32 = 151644;
pub const IM_END_ID: u32 = 151645;
pub const ENDOFTEXT_ID: u32 = 151643;
pub const AUDIO_START_ID: u32 = 151669;
pub const AUDIO_END_ID: u32 = 151670;
pub const AUDIO_PAD_ID: u32 = 151676;
pub const NEWLINE_ID: u32 = 198;

pub fn build_prompt_ids(
    n_audio_tokens: usize,
    language: Option<&str>,
    tokenizer: &dyn PromptTokenizer,
) -> Result<Vec<u32>, SttError> {
    build_prompt_ids_with_prefix(n_audio_tokens, language, "", tokenizer)
}

pub fn build_prompt_ids_with_prefix(
    n_audio_tokens: usize,
    language: Option<&str>,
    prefix: &str,
    tokenizer: &dyn PromptTokenizer,
) -> Result<Vec<u32>, SttError> {
    let mut ids = Vec::new();

    let system_tokens = tokenizer.encode("system")?;
    ids.push(IM_START_ID);
    ids.extend_from_slice(&system_tokens);
    ids.push(NEWLINE_ID);
    ids.push(IM_END_ID);
    ids.push(NEWLINE_ID);

    let user_tokens = tokenizer.encode("user")?;
    ids.push(IM_START_ID);
    ids.extend_from_slice(&user_tokens);
    ids.push(NEWLINE_ID);

    ids.push(AUDIO_START_ID);
    ids.extend(std::iter::repeat_n(AUDIO_PAD_ID, n_audio_tokens));
    ids.push(AUDIO_END_ID);

    ids.push(IM_END_ID);
    ids.push(NEWLINE_ID);

    let assistant_tokens = tokenizer.encode("assistant")?;
    ids.push(IM_START_ID);
    ids.extend_from_slice(&assistant_tokens);
    ids.push(NEWLINE_ID);

    if let Some(lang) = language {
        let lang_text = format!("language {}<asr_text>", lang);
        let lang_tokens = tokenizer.encode(&lang_text)?;
        ids.extend_from_slice(&lang_tokens);
    }

    if !prefix.is_empty() {
        let prefix_tokens = tokenizer.encode(prefix)?;
        ids.extend_from_slice(&prefix_tokens);
    }

    Ok(ids)
}

pub fn get_audio_pad_range(prompt_ids: &[u32]) -> Result<(usize, usize), SttError> {
    let Some(start) = prompt_ids.iter().position(|&id| id == AUDIO_PAD_ID) else {
        return Err(SttError::InferenceError {
            model: "prompt".into(),
            detail: "Prompt does not contain any <|audio_pad|> tokens".into(),
        });
    };

    let audio_pad_count = prompt_ids[start..]
        .iter()
        .take_while(|&&id| id == AUDIO_PAD_ID)
        .count();

    if audio_pad_count == 0 {
        return Err(SttError::InferenceError {
            model: "prompt".into(),
            detail: "Prompt audio pad range is empty".into(),
        });
    }

    Ok((start, start + audio_pad_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_token_constants() {
        assert_eq!(IM_START_ID, 151644);
        assert_eq!(IM_END_ID, 151645);
        assert_eq!(ENDOFTEXT_ID, 151643);
        assert_eq!(AUDIO_START_ID, 151669);
        assert_eq!(AUDIO_END_ID, 151670);
        assert_eq!(AUDIO_PAD_ID, 151676);
        assert_eq!(NEWLINE_ID, 198);
    }

    struct FakeTokenizer {
        encode_offset: u32,
    }

    impl FakeTokenizer {
        fn new() -> Self {
            Self {
                encode_offset: 100000,
            }
        }
    }

    impl PromptTokenizer for FakeTokenizer {
        fn encode(&self, _text: &str) -> Result<Vec<u32>, SttError> {
            Ok(vec![self.encode_offset, self.encode_offset + 1])
        }
    }

    fn build_prompt_ids_fake(
        n_audio_tokens: usize,
        language: Option<&str>,
    ) -> Result<Vec<u32>, SttError> {
        let fake = FakeTokenizer::new();
        build_prompt_ids(n_audio_tokens, language, &fake as &dyn PromptTokenizer)
    }

    fn build_prompt_ids_with_prefix_fake(
        n_audio_tokens: usize,
        language: Option<&str>,
        prefix: &str,
    ) -> Result<Vec<u32>, SttError> {
        let fake = FakeTokenizer::new();
        build_prompt_ids_with_prefix(
            n_audio_tokens,
            language,
            prefix,
            &fake as &dyn PromptTokenizer,
        )
    }

    #[test]
    fn test_build_prompt_ids_no_audio() {
        let ids = build_prompt_ids_fake(0, None).unwrap();

        assert_eq!(ids[0], IM_START_ID);
        assert!(!ids.contains(&AUDIO_PAD_ID));
    }

    #[test]
    fn test_build_prompt_ids_with_audio() {
        let n_audio = 10;
        let ids = build_prompt_ids_fake(n_audio, None).unwrap();

        let audio_pad_count = ids.iter().filter(|&&id| id == AUDIO_PAD_ID).count();
        assert_eq!(audio_pad_count, n_audio);
    }

    #[test]
    fn test_build_prompt_ids_very_large_audio() {
        let n_audio = 10000;
        let ids = build_prompt_ids_fake(n_audio, None).unwrap();

        let audio_pad_count = ids.iter().filter(|&&id| id == AUDIO_PAD_ID).count();
        assert_eq!(audio_pad_count, n_audio);
    }

    #[test]
    fn test_build_prompt_ids_with_language() {
        let ids = build_prompt_ids_fake(5, Some("zh")).unwrap();

        assert!(ids.len() > 20);
    }

    #[test]
    fn test_build_prompt_ids_with_prefix() {
        let without_prefix = build_prompt_ids_fake(5, Some("zh")).unwrap();
        let with_prefix = build_prompt_ids_with_prefix_fake(5, Some("zh"), "hello").unwrap();

        assert_eq!(with_prefix.len(), without_prefix.len() + 2);
    }

    #[test]
    fn test_build_prompt_ids_with_auto_prefix() {
        let without_prefix = build_prompt_ids_fake(5, None).unwrap();
        let with_prefix =
            build_prompt_ids_with_prefix_fake(5, None, "language English<asr_text>hello").unwrap();

        assert_eq!(with_prefix.len(), without_prefix.len() + 2);
    }

    #[test]
    fn test_build_prompt_ids_structure() {
        let ids = build_prompt_ids_fake(10, None).unwrap();

        assert_eq!(ids[0], IM_START_ID);

        let audio_start_pos = ids.iter().position(|&id| id == AUDIO_START_ID);
        let audio_end_pos = ids.iter().position(|&id| id == AUDIO_END_ID);
        assert!(audio_start_pos.is_some());
        assert!(audio_end_pos.is_some());
        assert!(audio_start_pos.unwrap() < audio_end_pos.unwrap());
    }

    #[test]
    fn test_get_audio_pad_range() {
        let ids = build_prompt_ids_fake(5, None).unwrap();

        let (start, end) = get_audio_pad_range(&ids).unwrap();
        assert_eq!(end - start, 5);
        assert!(ids[start..end].iter().all(|&id| id == AUDIO_PAD_ID));
    }

    proptest::proptest! {
        #[test]
        fn prop_prompt_starts_with_im_start(n_audio_tokens in 0usize..1000) {
            let ids = build_prompt_ids_fake(n_audio_tokens, None).unwrap();
            assert_eq!(ids[0], IM_START_ID);
        }

        #[test]
        fn prop_audio_pad_count_preserved(n_audio_tokens in 0usize..1000) {
            let ids = build_prompt_ids_fake(n_audio_tokens, None).unwrap();
            let audio_pad_count = ids.iter().filter(|&&id| id == AUDIO_PAD_ID).count();
            assert_eq!(audio_pad_count, n_audio_tokens);
        }

        #[test]
        fn prop_im_start_count(n_audio_tokens in 0usize..100) {
            let ids = build_prompt_ids_fake(n_audio_tokens, None).unwrap();
            let start_count = ids.iter().filter(|&&id| id == IM_START_ID).count();
            assert_eq!(start_count, 3);
        }

        #[test]
        fn prop_audio_tokens_surrounded(n_audio_tokens in 1usize..100) {
            let ids = build_prompt_ids_fake(n_audio_tokens, None).unwrap();
            let audio_start = ids.iter().position(|&id| id == AUDIO_START_ID);
            let audio_end = ids.iter().position(|&id| id == AUDIO_END_ID);
            assert!(audio_start.is_some());
            assert!(audio_end.is_some());
            assert!(audio_start.unwrap() < audio_end.unwrap());
        }
    }
}
