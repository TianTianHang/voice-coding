const ASR_TEXT_TAG: &str = "<asr_text>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedQwen3Output {
    pub text: String,
    pub language: String,
}

pub(crate) fn parse_qwen3_output(
    decoded: &str,
    forced_language: Option<&str>,
) -> ParsedQwen3Output {
    if let Some(language) = forced_language {
        return ParsedQwen3Output {
            text: decoded.trim().to_string(),
            language: language.to_string(),
        };
    }

    let Some((metadata, text)) = decoded.split_once(ASR_TEXT_TAG) else {
        return ParsedQwen3Output {
            text: decoded.trim().to_string(),
            language: "auto".to_string(),
        };
    };

    ParsedQwen3Output {
        text: text.trim().to_string(),
        language: extract_language_label(metadata)
            .and_then(normalize_language_label)
            .unwrap_or_else(|| "auto".to_string()),
    }
}

fn extract_language_label(metadata: &str) -> Option<&str> {
    let language_index = metadata.rfind("language")?;
    let label = metadata[language_index + "language".len()..].trim();

    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

fn normalize_language_label(label: &str) -> Option<String> {
    let normalized = label.trim().to_ascii_lowercase().replace(['_', '-'], " ");
    let code = match normalized.as_str() {
        "none" => return None,
        "zh" | "chinese" | "mandarin" => "zh",
        "en" | "english" => "en",
        "yue" | "cantonese" => "yue",
        "ja" | "japanese" => "ja",
        "ko" | "korean" => "ko",
        "ar" | "arabic" => "ar",
        "de" | "german" => "de",
        "fr" | "french" => "fr",
        "es" | "spanish" => "es",
        "pt" | "portuguese" => "pt",
        "id" | "indonesian" => "id",
        "it" | "italian" => "it",
        "ru" | "russian" => "ru",
        "th" | "thai" => "th",
        "vi" | "vietnamese" => "vi",
        "tr" | "turkish" => "tr",
        "hi" | "hindi" => "hi",
        "ms" | "malay" => "ms",
        "nl" | "dutch" => "nl",
        "sv" | "swedish" => "sv",
        "da" | "danish" => "da",
        "fi" | "finnish" => "fi",
        "pl" | "polish" => "pl",
        "cz" | "czech" => "cz",
        "fil" | "filipino" => "fil",
        "fa" | "persian" | "farsi" => "fa",
        "el" | "greek" => "el",
        "ro" | "romanian" => "ro",
        "hu" | "hungarian" => "hu",
        "mk" | "macedonian" => "mk",
        _ => return None,
    };

    Some(code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tagged_auto_output() {
        let parsed = parse_qwen3_output("language Chinese<asr_text>  hello world  ", None);

        assert_eq!(parsed.text, "hello world");
        assert_eq!(parsed.language, "zh");
    }

    #[test]
    fn parses_newline_metadata() {
        let parsed = parse_qwen3_output("language\nEnglish\n<asr_text>\nhello\n", None);

        assert_eq!(parsed.text, "hello");
        assert_eq!(parsed.language, "en");
    }

    #[test]
    fn preserves_no_tag_auto_output() {
        let parsed = parse_qwen3_output("  plain transcript  ", None);

        assert_eq!(parsed.text, "plain transcript");
        assert_eq!(parsed.language, "auto");
    }

    #[test]
    fn treats_forced_language_output_as_text_only() {
        let parsed = parse_qwen3_output(
            "  language Chinese<asr_text> should stay visible  ",
            Some("en"),
        );

        assert_eq!(
            parsed.text,
            "language Chinese<asr_text> should stay visible"
        );
        assert_eq!(parsed.language, "en");
    }

    #[test]
    fn handles_empty_audio_metadata() {
        let parsed = parse_qwen3_output("language None<asr_text>   ", None);

        assert_eq!(parsed.text, "");
        assert_eq!(parsed.language, "auto");
    }

    #[test]
    fn handles_none_language_with_returned_text() {
        let parsed = parse_qwen3_output("language None<asr_text>  recovered text  ", None);

        assert_eq!(parsed.text, "recovered text");
        assert_eq!(parsed.language, "auto");
    }

    #[test]
    fn normalizes_supported_language_labels_conservatively() {
        let labels = [
            ("Cantonese", "yue"),
            ("Japanese", "ja"),
            ("Korean", "ko"),
            ("Arabic", "ar"),
            ("German", "de"),
            ("French", "fr"),
            ("Spanish", "es"),
            ("Portuguese", "pt"),
            ("Indonesian", "id"),
            ("Italian", "it"),
            ("Russian", "ru"),
            ("Thai", "th"),
            ("Vietnamese", "vi"),
            ("Turkish", "tr"),
            ("Hindi", "hi"),
            ("Malay", "ms"),
            ("Dutch", "nl"),
            ("Swedish", "sv"),
            ("Danish", "da"),
            ("Finnish", "fi"),
            ("Polish", "pl"),
            ("Czech", "cz"),
            ("Filipino", "fil"),
            ("Persian", "fa"),
            ("Greek", "el"),
            ("Romanian", "ro"),
            ("Hungarian", "hu"),
            ("Macedonian", "mk"),
        ];

        for (label, expected) in labels {
            let parsed = parse_qwen3_output(&format!("language {label}<asr_text>x"), None);
            assert_eq!(parsed.language, expected, "label {label}");
        }
    }
}
