#[derive(Debug, Clone)]
pub struct TimingInfo {
    pub audio_duration_sec: f64,
    pub processing_time_sec: f64,
    pub rtf: f64,
    pub tokens_generated: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SttResult {
    pub text: String,
    pub language: String,
    pub confidence: Option<f64>,
    pub timing: TimingInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_info_carries_rtf_and_token_count() {
        let timing = TimingInfo {
            audio_duration_sec: 10.0,
            processing_time_sec: 3.2,
            rtf: 0.32,
            tokens_generated: Some(45),
        };
        assert_eq!(timing.rtf, 0.32);
        assert!(timing.rtf < 1.0);
        assert_eq!(timing.tokens_generated, Some(45));
    }
}
