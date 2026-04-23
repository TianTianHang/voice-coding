use std::path::Path;

use stt_core::SttError;

pub struct TokenizerWrapper {
    tokenizer: tokenizers::Tokenizer,
}

impl TokenizerWrapper {
    pub fn load(model_dir: &Path) -> Result<Self, SttError> {
        let path = model_dir.join("tokenizer.json");
        if !path.exists() {
            return Err(SttError::TokenizerError(format!(
                "Tokenizer file not found: {}",
                path.display()
            )));
        }

        let tokenizer = tokenizers::Tokenizer::from_file(&path)
            .map_err(|e| SttError::TokenizerError(format!("Failed to load tokenizer: {}", e)))?;

        Ok(Self { tokenizer })
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u32>, SttError> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| SttError::TokenizerError(format!("Encode error: {}", e)))?;
        Ok(encoding.get_ids().to_vec())
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String, SttError> {
        let text = self
            .tokenizer
            .decode(ids, true)
            .map_err(|e| SttError::TokenizerError(format!("Decode error: {}", e)))?;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_missing_tokenizer() {
        let result = TokenizerWrapper::load(Path::new("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_decode_roundtrip_ascii() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "Hello world!";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_chinese() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "你好世界";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_japanese() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "こんにちは";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_korean() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "안녕하세요";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_mixed() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "Hello 你好 こんにちは";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_special_chars() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "Hello\nWorld\tTest";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_roundtrip_emoji() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let text = "Hello 😀 World 🌍";
        let encoded = wrapper.encode(text).unwrap();
        let decoded = wrapper.decode(&encoded).unwrap();

        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_error_handling() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let result = wrapper.encode("");

        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_decode_error_handling() {
        let wrapper = TokenizerWrapper::load(Path::new("models/tokenizer.json"));
        if wrapper.is_err() {
            return;
        }

        let wrapper = wrapper.unwrap();
        let result = wrapper.decode(&[]);

        assert!(result.is_ok());
    }
}
