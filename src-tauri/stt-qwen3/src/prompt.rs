use stt_core::SttError;
use crate::tokenizer::wrapper::TokenizerWrapper;

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
    tokenizer: &TokenizerWrapper,
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

    Ok(ids)
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
}
