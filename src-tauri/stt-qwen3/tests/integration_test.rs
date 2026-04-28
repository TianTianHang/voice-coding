use std::path::Path;
use stt_core::{AudioInput, SttConfig, SttEngine};

fn model_tests_enabled() -> bool {
    std::env::var("RUN_QWEN3_MODEL_TESTS")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

#[tokio::test]
async fn test_stt_qwen3() -> Result<(), Box<dyn std::error::Error>> {
    if !model_tests_enabled() {
        eprintln!("Skipping model inference test; set RUN_QWEN3_MODEL_TESTS=1 to enable");
        return Ok(());
    }

    println!("🎙️  STT Engine Integration Test");
    println!("==============================\n");

    let model_dir = "../../models";

    println!("📂 Loading STT engine from: {}", model_dir);
    let engine = stt_qwen3::Qwen3AsrEngine::new(&model_dir)?;
    println!("✅ Engine loaded: {}", engine.engine_name());
    println!("🌍 Supported languages: {:?}", engine.supported_languages());

    // Health check
    println!("\n🏥 Running health check...");
    let health = engine.health_check().await?;
    println!("✅ Health check: {}", health);

    // Test audio files
    let test_audio_dir = "../../test_audio";
    let test_files = [
        "librispeech_0_1089_0.wav",
        "librispeech_1_1089_1.wav",
        "librispeech_2_1089_2.wav",
    ];

    println!(
        "\n🎵 Testing transcription on {} audio files...\n",
        test_files.len()
    );

    for (idx, filename) in test_files.iter().enumerate() {
        let audio_path = Path::new(test_audio_dir).join(filename);

        if !audio_path.exists() {
            println!("❌ [{idx}] File not found: {}", audio_path.display());
            continue;
        }

        println!("📄 [{idx}] Testing: {}", filename);

        let config = SttConfig {
            language: Some("en".to_string()),
            ..Default::default()
        };

        let input = AudioInput::FilePath(audio_path.to_string_lossy().to_string());

        match engine.transcribe(input, config).await {
            Ok(result) => {
                println!("✅ [{idx}] Transcription successful!");
                println!("   📝 Text: {}", result.text);
                println!("   🌍 Language: {}", result.language);
                println!(
                    "   ⏱️  Audio: {:.2}s | Process: {:.2}s | RTF: {:.3}",
                    result.timing.audio_duration_sec,
                    result.timing.processing_time_sec,
                    result.timing.rtf
                );
                println!("   🔢 Tokens: {:?}", result.timing.tokens_generated);
            }
            Err(e) => {
                println!("❌ [{idx}] Transcription failed: {}", e);
            }
        }
        println!();
    }

    println!("==============================");
    println!("✨ Test complete!");

    Ok(())
}
