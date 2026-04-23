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

        let tokenizer = tokenizers::Tokenizer::from_file(&path).map_err(|e| {
            SttError::TokenizerError(format!("Failed to load tokenizer: {}", e))
        })?;

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
}
