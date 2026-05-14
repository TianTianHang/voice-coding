use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;
use stt_core::{AudioInput, SttConfig, SttEngine};
use stt_qwen3::Qwen3AsrEngine;

const DEFAULT_MODEL_HOME: &str = "../../models";
const QWEN3_ASR_MODEL_ID: &str = "qwen3-asr-0.6b-onnx";
const WARMUP_DURATION_SEC: usize = 2;
const MEASUREMENT_RUNS: usize = 3;
const DEFAULT_WER_SAMPLE_LIMIT: usize = 200;
const QWEN3_ASR_INT4_REFERENCE_WER: f64 = 5.16;

#[derive(Debug, Clone)]
struct Measurement {
    wall_ms: u128,
    process_ms: u128,
    rtf: f64,
    tokens: usize,
    chars: usize,
}

#[derive(Debug, Clone)]
struct WerSample {
    audio_path: PathBuf,
    reference: String,
}

#[derive(Debug, Default)]
struct EditDistance {
    distance: usize,
    reference_len: usize,
}

#[derive(Debug, Clone)]
struct WerSampleResult {
    label: String,
    duration_sec: f64,
    wall_ms: u128,
    process_ms: u128,
    rtf: f64,
    word_errors: usize,
    reference_words: usize,
    char_errors: usize,
    reference_chars: usize,
    hypothesis: String,
}

fn model_tests_enabled() -> bool {
    std::env::var("RUN_QWEN3_MODEL_TESTS")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn wer_manifest_path() -> Option<PathBuf> {
    env_path("QWEN3_WER_MANIFEST")
}

fn wer_sample_limit() -> usize {
    std::env::var("QWEN3_WER_N_SAMPLES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_WER_SAMPLE_LIMIT)
}

fn create_mock_samples(duration_sec: usize, sample_rate: u32) -> Vec<f32> {
    let num_samples = duration_sec * sample_rate as usize;
    (0..num_samples)
        .map(|i| {
            let freq = 440.0;
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32;
            phase.sin() * 0.3
        })
        .collect()
}

fn resolve_model_dir() -> PathBuf {
    if let Some(model_dir) = env_path("STT_MODEL_DIR") {
        return model_dir;
    }

    if let Some(model_home) = env_path("VOICE_CODING_MODEL_HOME") {
        return standard_model_dir(model_home);
    }

    let default_home = PathBuf::from(DEFAULT_MODEL_HOME);
    let standard = standard_model_dir(&default_home);
    if has_any_model_asset(&standard) {
        return standard;
    }

    default_home
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn standard_model_dir(model_home: impl AsRef<std::path::Path>) -> PathBuf {
    model_home.as_ref().join("asr").join(QWEN3_ASR_MODEL_ID)
}

fn has_any_model_asset(model_dir: &std::path::Path) -> bool {
    model_dir.join("tokenizer.json").exists() || model_dir.join("onnx_models").exists()
}

fn fmt_secs(value: f64) -> String {
    format!("{value:.3}s")
}

fn fmt_ms(value: u128) -> String {
    format!("{value:.2}ms")
}

fn percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 * 100.0 / denominator as f64
    }
}

fn normalize_for_error_rate(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '\'' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn edit_distance<T: Eq>(reference: &[T], hypothesis: &[T]) -> EditDistance {
    let mut prev: Vec<usize> = (0..=hypothesis.len()).collect();
    let mut curr = vec![0; hypothesis.len() + 1];

    for (i, reference_item) in reference.iter().enumerate() {
        curr[0] = i + 1;
        for (j, hypothesis_item) in hypothesis.iter().enumerate() {
            let substitution_cost = usize::from(reference_item != hypothesis_item);
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + substitution_cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    EditDistance {
        distance: prev[hypothesis.len()],
        reference_len: reference.len(),
    }
}

fn word_error_distance(reference: &str, hypothesis: &str) -> EditDistance {
    let normalized_reference = normalize_for_error_rate(reference);
    let normalized_hypothesis = normalize_for_error_rate(hypothesis);
    let reference_words = normalized_reference
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let hypothesis_words = normalized_hypothesis
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    edit_distance(&reference_words, &hypothesis_words)
}

fn char_error_distance(reference: &str, hypothesis: &str) -> EditDistance {
    let normalized_reference = normalize_for_error_rate(reference).replace(' ', "");
    let normalized_hypothesis = normalize_for_error_rate(hypothesis).replace(' ', "");
    let reference_chars = normalized_reference.chars().collect::<Vec<_>>();
    let hypothesis_chars = normalized_hypothesis.chars().collect::<Vec<_>>();

    edit_distance(&reference_chars, &hypothesis_chars)
}

fn json_string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn resolve_sample_audio_path(manifest_dir: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}

fn read_wer_manifest(manifest_path: &Path, sample_limit: usize) -> Vec<WerSample> {
    let manifest = fs::read_to_string(manifest_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read QWEN3_WER_MANIFEST {}: {e}",
            manifest_path.display()
        )
    });
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut samples = Vec::new();

    for (line_index, line) in manifest.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let value = serde_json::from_str::<Value>(line).unwrap_or_else(|e| {
            panic!(
                "Invalid JSONL at {}:{}: {e}",
                manifest_path.display(),
                line_index + 1
            )
        });
        let audio_path = json_string_field(
            &value,
            &["audio_filepath", "audio_path", "audio", "path", "file"],
        )
        .unwrap_or_else(|| {
            panic!(
                "Missing audio path field at {}:{}",
                manifest_path.display(),
                line_index + 1
            )
        });
        let reference = json_string_field(
            &value,
            &["text", "reference", "transcript", "sentence", "target"],
        )
        .unwrap_or_else(|| {
            panic!(
                "Missing reference text field at {}:{}",
                manifest_path.display(),
                line_index + 1
            )
        });

        samples.push(WerSample {
            audio_path: resolve_sample_audio_path(manifest_dir, audio_path),
            reference: reference.to_string(),
        });

        if samples.len() == sample_limit {
            break;
        }
    }

    samples
}

async fn measure_transcription(
    engine: &Qwen3AsrEngine,
    label: &str,
    duration_sec: usize,
    sample_rate: u32,
    enable_vad: bool,
) -> Measurement {
    let samples = create_mock_samples(duration_sec, sample_rate);
    let input = AudioInput::Samples(samples, sample_rate);
    let config = SttConfig {
        enable_vad,
        chunk_seconds: Some(30.0),
        max_new_tokens: Some(32),
        ..Default::default()
    };

    let wall_start = Instant::now();
    let result = engine
        .transcribe(input, config)
        .await
        .unwrap_or_else(|e| panic!("{label} transcription failed: {e}"));
    let wall_ms = wall_start.elapsed().as_millis();

    Measurement {
        wall_ms,
        process_ms: (result.timing.processing_time_sec * 1000.0) as u128,
        rtf: result.timing.rtf,
        tokens: result.timing.tokens_generated.unwrap_or(0),
        chars: result.text.chars().count(),
    }
}

async fn measure_wer_sample(
    engine: &Qwen3AsrEngine,
    sample: &WerSample,
    index: usize,
) -> WerSampleResult {
    let input = AudioInput::FilePath(sample.audio_path.to_string_lossy().into_owned());
    let config = SttConfig {
        language: Some("en".to_string()),
        enable_vad: false,
        chunk_seconds: Some(30.0),
        max_new_tokens: Some(512),
        ..Default::default()
    };

    let wall_start = Instant::now();
    let result = engine
        .transcribe(input, config)
        .await
        .unwrap_or_else(|e| panic!("WER sample {} transcription failed: {e}", index + 1));
    let wall_ms = wall_start.elapsed().as_millis();
    let word_distance = word_error_distance(&sample.reference, &result.text);
    let char_distance = char_error_distance(&sample.reference, &result.text);

    WerSampleResult {
        label: sample
            .audio_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_string(),
        duration_sec: result.timing.audio_duration_sec,
        wall_ms,
        process_ms: (result.timing.processing_time_sec * 1000.0) as u128,
        rtf: result.timing.rtf,
        word_errors: word_distance.distance,
        reference_words: word_distance.reference_len,
        char_errors: char_distance.distance,
        reference_chars: char_distance.reference_len,
        hypothesis: result.text,
    }
}

fn average_u128(values: impl Iterator<Item = u128>, len: usize) -> u128 {
    values.sum::<u128>() / len as u128
}

fn average_f64(values: impl Iterator<Item = f64>, len: usize) -> f64 {
    values.sum::<f64>() / len as f64
}

#[tokio::test]
#[ignore]
async fn test_qwen3_performance_report() {
    if !model_tests_enabled() {
        eprintln!("Skipping performance report; set RUN_QWEN3_MODEL_TESTS=1 to enable");
        return;
    }

    println!("========================================");
    println!("Qwen3 ASR Performance Report");
    println!("========================================");
    let model_dir = resolve_model_dir();
    let model_dir_display = model_dir.display();
    println!("Model dir: {model_dir_display}");
    println!();

    let load_wall_start = Instant::now();
    let engine =
        Qwen3AsrEngine::new(&model_dir.to_string_lossy()).expect("Failed to create test engine");
    let load_wall_ms = load_wall_start.elapsed().as_millis();
    let load_timing = engine.load_timing();

    println!("Model load");
    println!("  wall clock      : {}", fmt_ms(load_wall_ms));
    println!(
        "  total           : {}",
        fmt_ms(load_timing.total_ms as u128)
    );
    println!(
        "  onnx sessions   : {}",
        fmt_ms(load_timing.onnx_sessions_ms as u128)
    );
    println!(
        "  embeddings      : {}",
        fmt_ms(load_timing.embeddings_ms as u128)
    );
    println!(
        "  tokenizer       : {}",
        fmt_ms(load_timing.tokenizer_ms as u128)
    );
    println!(
        "  mel filterbank  : {}",
        fmt_ms(load_timing.mel_filterbank_ms as u128)
    );
    println!();

    let health_start = Instant::now();
    let health = engine
        .health_check()
        .await
        .expect("health check should succeed");
    println!(
        "Health check     : {health} ({})",
        fmt_ms(health_start.elapsed().as_millis())
    );
    println!();

    println!(
        "Warmup          : {} @ 16kHz (not included in measurements)",
        fmt_secs(WARMUP_DURATION_SEC as f64)
    );
    let warmup =
        measure_transcription(&engine, "warmup-16k-2s", WARMUP_DURATION_SEC, 16000, false).await;
    println!(
        "  wall/process   : {} / {}, rtf {:.3}, tokens {}",
        fmt_ms(warmup.wall_ms),
        fmt_ms(warmup.process_ms),
        warmup.rtf,
        warmup.tokens
    );
    println!();

    println!("Measurements    : {MEASUREMENT_RUNS} runs per case after warmup");
    println!(
        "{:<22} {:>8} {:>12} {:>12} {:>8} {:>12} {:>10} {:>10}",
        "case", "audio", "wall_avg", "process_avg", "rtf_avg", "wall_min/max", "tokens", "chars"
    );
    println!("{}", "-".repeat(112));

    let cases = [
        ("16k-2s", 2usize, 16000u32, false),
        ("48k-2s", 2usize, 48000u32, false),
        ("16k-10s", 10usize, 16000u32, false),
        ("16k-45s-vad", 45usize, 16000u32, true),
    ];

    for (label, duration_sec, sample_rate, enable_vad) in cases {
        let mut measurements = Vec::with_capacity(MEASUREMENT_RUNS);
        for _ in 0..MEASUREMENT_RUNS {
            measurements.push(
                measure_transcription(&engine, label, duration_sec, sample_rate, enable_vad).await,
            );
        }

        let wall_avg = average_u128(measurements.iter().map(|m| m.wall_ms), measurements.len());
        let process_avg = average_u128(
            measurements.iter().map(|m| m.process_ms),
            measurements.len(),
        );
        let rtf_avg = average_f64(measurements.iter().map(|m| m.rtf), measurements.len());
        let wall_min = measurements.iter().map(|m| m.wall_ms).min().unwrap_or(0);
        let wall_max = measurements.iter().map(|m| m.wall_ms).max().unwrap_or(0);
        let tokens_avg = average_u128(
            measurements.iter().map(|m| m.tokens as u128),
            measurements.len(),
        );
        let chars_avg = average_u128(
            measurements.iter().map(|m| m.chars as u128),
            measurements.len(),
        );

        println!(
            "{:<22} {:>8} {:>12} {:>12} {:>8.3} {:>12} {:>10} {:>10}",
            label,
            fmt_secs(duration_sec as f64),
            fmt_ms(wall_avg),
            fmt_ms(process_avg),
            rtf_avg,
            format!("{}/{}", fmt_ms(wall_min), fmt_ms(wall_max)),
            tokens_avg,
            chars_avg,
        );
    }

    println!("{}", "-".repeat(112));
    println!("Run with: RUN_QWEN3_MODEL_TESTS=1 cargo test --test performance_test -- --ignored --nocapture");
}

#[tokio::test]
#[ignore]
async fn test_qwen3_librispeech_other_wer_report() {
    if !model_tests_enabled() {
        eprintln!("Skipping WER report; set RUN_QWEN3_MODEL_TESTS=1 to enable");
        return;
    }

    let Some(manifest_path) = wer_manifest_path() else {
        eprintln!(
            "Skipping WER report; set QWEN3_WER_MANIFEST to a JSONL manifest of LibriSpeech other samples"
        );
        return;
    };

    let sample_limit = wer_sample_limit();
    let samples = read_wer_manifest(&manifest_path, sample_limit);
    assert!(
        !samples.is_empty(),
        "QWEN3_WER_MANIFEST {} did not contain any samples",
        manifest_path.display()
    );

    println!("========================================");
    println!("Qwen3 ASR LibriSpeech Other WER Report");
    println!("========================================");
    println!("Reference script : andrewleech/qwen3-asr-onnx evaluate_wer.py");
    println!("Dataset target   : librispeech-other");
    println!("Requested samples: {sample_limit}");
    println!("Loaded samples   : {}", samples.len());
    println!("Manifest         : {}", manifest_path.display());
    println!(
        "Reference WER    : {:.2}% (0.6B int4 report)",
        QWEN3_ASR_INT4_REFERENCE_WER
    );
    println!();

    let model_dir = resolve_model_dir();
    let model_dir_display = model_dir.display();
    println!("Model dir        : {model_dir_display}");
    println!();

    let load_wall_start = Instant::now();
    let engine =
        Qwen3AsrEngine::new(&model_dir.to_string_lossy()).expect("Failed to create test engine");
    let load_wall_ms = load_wall_start.elapsed().as_millis();
    let load_timing = engine.load_timing();

    println!("Model load");
    println!("  wall clock      : {}", fmt_ms(load_wall_ms));
    println!(
        "  total           : {}",
        fmt_ms(load_timing.total_ms as u128)
    );
    println!(
        "  onnx sessions   : {}",
        fmt_ms(load_timing.onnx_sessions_ms as u128)
    );
    println!(
        "  embeddings      : {}",
        fmt_ms(load_timing.embeddings_ms as u128)
    );
    println!(
        "  tokenizer       : {}",
        fmt_ms(load_timing.tokenizer_ms as u128)
    );
    println!();

    println!(
        "Warmup           : {} @ 16kHz synthetic audio (not included in WER/RTF)",
        fmt_secs(WARMUP_DURATION_SEC as f64)
    );
    let warmup = measure_transcription(
        &engine,
        "wer-warmup-16k-2s",
        WARMUP_DURATION_SEC,
        16000,
        false,
    )
    .await;
    println!(
        "  wall/process    : {} / {}, rtf {:.3}, tokens {}",
        fmt_ms(warmup.wall_ms),
        fmt_ms(warmup.process_ms),
        warmup.rtf,
        warmup.tokens
    );
    println!();

    println!(
        "{:<6} {:<28} {:>8} {:>10} {:>10} {:>8} {:>9} {:>9}",
        "idx", "audio", "dur", "wall", "process", "rtf", "WER", "CER"
    );
    println!("{}", "-".repeat(100));

    let mut results = Vec::with_capacity(samples.len());
    for (index, sample) in samples.iter().enumerate() {
        let result = measure_wer_sample(&engine, sample, index).await;
        println!(
            "{:<6} {:<28} {:>8} {:>10} {:>10} {:>8.3} {:>8.2}% {:>8.2}%",
            index + 1,
            result.label,
            fmt_secs(result.duration_sec),
            fmt_ms(result.wall_ms),
            fmt_ms(result.process_ms),
            result.rtf,
            percent(result.word_errors, result.reference_words),
            percent(result.char_errors, result.reference_chars)
        );
        results.push(result);
    }

    let total_audio_sec = results
        .iter()
        .map(|result| result.duration_sec)
        .sum::<f64>();
    let total_wall_ms = results.iter().map(|result| result.wall_ms).sum::<u128>();
    let total_process_ms = results.iter().map(|result| result.process_ms).sum::<u128>();
    let total_word_errors = results
        .iter()
        .map(|result| result.word_errors)
        .sum::<usize>();
    let total_reference_words = results
        .iter()
        .map(|result| result.reference_words)
        .sum::<usize>();
    let total_char_errors = results
        .iter()
        .map(|result| result.char_errors)
        .sum::<usize>();
    let total_reference_chars = results
        .iter()
        .map(|result| result.reference_chars)
        .sum::<usize>();
    let aggregate_wer = percent(total_word_errors, total_reference_words);
    let aggregate_cer = percent(total_char_errors, total_reference_chars);
    let wall_rtf = if total_audio_sec == 0.0 {
        0.0
    } else {
        total_wall_ms as f64 / 1000.0 / total_audio_sec
    };
    let process_rtf = if total_audio_sec == 0.0 {
        0.0
    } else {
        total_process_ms as f64 / 1000.0 / total_audio_sec
    };

    println!("{}", "-".repeat(100));
    println!("Summary");
    println!("  samples         : {}", results.len());
    println!("  audio duration  : {}", fmt_secs(total_audio_sec));
    println!("  wall time       : {}", fmt_ms(total_wall_ms));
    println!("  process time    : {}", fmt_ms(total_process_ms));
    println!("  wall/process RTF: {:.3} / {:.3}", wall_rtf, process_rtf);
    println!(
        "  WER             : {:.2}% ({}/{})",
        aggregate_wer, total_word_errors, total_reference_words
    );
    println!(
        "  CER             : {:.2}% ({}/{})",
        aggregate_cer, total_char_errors, total_reference_chars
    );
    println!(
        "  vs 5.16% report : {:+.2} percentage points",
        aggregate_wer - QWEN3_ASR_INT4_REFERENCE_WER
    );

    let mut worst = results.clone();
    worst.sort_by(|left, right| {
        let left_wer = percent(left.word_errors, left.reference_words);
        let right_wer = percent(right.word_errors, right.reference_words);
        right_wer
            .partial_cmp(&left_wer)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!();
    println!("Worst samples by WER");
    for result in worst.iter().take(5) {
        println!(
            "  {:<28} WER {:>6.2}% CER {:>6.2}% | hyp: {}",
            result.label,
            percent(result.word_errors, result.reference_words),
            percent(result.char_errors, result.reference_chars),
            result.hypothesis
        );
    }

    println!();
    println!(
        "Run with: RUN_QWEN3_MODEL_TESTS=1 QWEN3_WER_MANIFEST=/path/to/librispeech-other-200.jsonl cargo test --test performance_test test_qwen3_librispeech_other_wer_report -- --ignored --nocapture"
    );
}
