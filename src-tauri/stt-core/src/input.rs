#[derive(Debug, Clone)]
pub enum AudioInput {
    FilePath(String),
    Bytes(Vec<u8>),
    Samples(Vec<f32>, u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_supported_input_variants() {
        let file_input = AudioInput::FilePath("/path/to/audio.wav".into());
        let bytes_input = AudioInput::Bytes(vec![0u8; 1024]);
        let samples_input = AudioInput::Samples(vec![0.5f32; 16000], 16000);

        match file_input {
            AudioInput::FilePath(p) => assert_eq!(p, "/path/to/audio.wav"),
            _ => panic!("Expected FilePath variant"),
        }
        match bytes_input {
            AudioInput::Bytes(b) => assert_eq!(b.len(), 1024),
            _ => panic!("Expected Bytes variant"),
        }
        match samples_input {
            AudioInput::Samples(s, r) => {
                assert_eq!(s.len(), 16000);
                assert_eq!(r, 16000);
            }
            _ => panic!("Expected Samples variant"),
        }
    }
}
