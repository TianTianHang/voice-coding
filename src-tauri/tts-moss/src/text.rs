use crate::robust::normalize_robust_text;

const DEFAULT_MAX_TOKENS_PER_CHUNK: usize = 75;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedTextChunk {
    pub text: String,
    pub token_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub(crate) struct MossTextPreprocessor {
    max_tokens_per_chunk: usize,
}

impl Default for MossTextPreprocessor {
    fn default() -> Self {
        Self {
            max_tokens_per_chunk: DEFAULT_MAX_TOKENS_PER_CHUNK,
        }
    }
}

impl MossTextPreprocessor {
    #[cfg(test)]
    pub(crate) fn new(max_tokens_per_chunk: usize) -> Self {
        Self {
            max_tokens_per_chunk: max_tokens_per_chunk.max(1),
        }
    }

    pub(crate) fn normalize(&self, text: &str) -> String {
        normalize_tts_text(text)
    }

    pub(crate) fn prepare<F, E>(
        &self,
        text: &str,
        mut encode: F,
    ) -> Result<Vec<PreparedTextChunk>, E>
    where
        F: FnMut(&str) -> Result<Vec<i64>, E>,
    {
        let normalized = self.normalize(text);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        for sentence in sentence_segments(&normalized) {
            let token_ids = encode(sentence)?;
            if token_ids.len() <= self.max_tokens_per_chunk {
                push_preencoded(&mut chunks, sentence, token_ids);
            } else {
                split_oversized_sentence(
                    sentence,
                    self.max_tokens_per_chunk,
                    &mut encode,
                    &mut chunks,
                )?;
            }
        }
        merge_prepared_chunks(chunks, self.max_tokens_per_chunk, &mut encode)
    }
}

fn normalize_tts_text(text: &str) -> String {
    let language = resolve_text_language(text, "");
    let wetext_ready = match language {
        TextLanguage::Zh => rewrite_hyphens_before_zh_wetext(text),
        TextLanguage::En => text.to_string(),
    };
    let robust_pre = normalize_robust_text(&wetext_ready);
    let language = resolve_text_language(&robust_pre, "");
    let text = robust_pre;
    let mut normalized = String::with_capacity(text.len());
    let mut last_was_space = true;

    for raw in text.trim().chars() {
        let ch = normalize_char(raw);
        if ch.is_whitespace() {
            if !last_was_space {
                normalized.push(' ');
                last_was_space = true;
            }
            continue;
        }
        normalized.push(ch);
        last_was_space = false;
    }

    let collapsed = normalized.trim();
    let mut output = String::with_capacity(collapsed.len());
    let mut previous = None;
    let mut chars = collapsed.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ' ' {
            if let (Some(prev), Some(next)) = (previous, chars.peek().copied()) {
                if is_opening_punctuation(prev) || is_tight_punctuation(next) {
                    continue;
                }
            }
        }
        if let (Some(prev), true) = (previous, is_ascii_alnum(ch)) {
            if is_cjk(prev) {
                output.push(' ');
            }
        }
        if let (Some(prev), true) = (previous, is_cjk(ch)) {
            if is_ascii_alnum(prev) {
                output.push(' ');
            }
        }
        output.push(ch);
        previous = Some(ch);
    }
    let output = prepare_text_for_sentence_chunking(output, language);
    if has_speakable_content(&output) {
        output
    } else {
        String::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextLanguage {
    Zh,
    En,
}

fn resolve_text_language(text: &str, voice: &str) -> TextLanguage {
    if text.chars().any(is_cjk) {
        return TextLanguage::Zh;
    }
    if text.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return TextLanguage::En;
    }
    if matches!(voice, "Trump" | "Ava" | "Bella" | "Adam" | "Nathan") {
        TextLanguage::En
    } else {
        TextLanguage::Zh
    }
}

fn rewrite_hyphens_before_zh_wetext(text: &str) -> String {
    if !text.contains('-') {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch != '-' {
            output.push(ch);
            continue;
        }

        let previous = previous_non_space(&chars, index);
        let next = next_non_space(&chars, index);
        if next.is_some_and(|next| next.is_ascii_digit())
            && (index == 0
                || previous.is_none()
                || previous.is_some_and(|prev| {
                    prev.is_ascii_digit()
                        || is_cjk(prev)
                        || matches!(prev, '=' | ':' | '+' | '*' | '/' | ',' | '(' | '[' | '{')
                }))
        {
            if previous.is_some_and(is_cjk) {
                output.push(' ');
            }
            output.push('-');
        } else if previous.is_some_and(is_cjk) && next.is_some_and(is_cjk) {
            output.push(',');
        } else if previous.is_some_and(|prev| !prev.is_whitespace())
            && next.is_some_and(|next| !next.is_whitespace())
        {
            output.push(' ');
        } else {
            output.push(ch);
        }
    }

    collapse_ascii_spaces(&output)
}

fn previous_non_space(chars: &[char], index: usize) -> Option<char> {
    chars[..index]
        .iter()
        .rev()
        .copied()
        .find(|ch| !ch.is_whitespace())
}

fn next_non_space(chars: &[char], index: usize) -> Option<char> {
    chars[index + 1..]
        .iter()
        .copied()
        .find(|ch| !ch.is_whitespace())
}

fn collapse_ascii_spaces(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut last_was_space = true;
    for ch in text.chars() {
        if ch == ' ' {
            if !last_was_space {
                output.push(ch);
            }
            last_was_space = true;
        } else {
            output.push(ch);
            last_was_space = false;
        }
    }
    output.trim().to_string()
}

fn prepare_text_for_sentence_chunking(mut text: String, language: TextLanguage) -> String {
    if text.is_empty() {
        return text;
    }
    match language {
        TextLanguage::Zh => {
            if !text.ends_with(is_terminal_sentence_punctuation) {
                text.push('\u{3002}');
            }
            text
        }
        TextLanguage::En => {
            if let Some((index, ch)) = text.char_indices().find(|(_, ch)| ch.is_alphabetic()) {
                if ch.is_lowercase() {
                    let upper = ch.to_uppercase().to_string();
                    text.replace_range(index..index + ch.len_utf8(), &upper);
                }
            }
            if text.ends_with(|ch: char| ch.is_alphanumeric()) {
                text.push('.');
            }
            if text.split_whitespace().count() < 5 {
                format!(" {text}")
            } else {
                text
            }
        }
    }
}

fn normalize_char(ch: char) -> char {
    match ch {
        '\u{3000}' => ' ',
        '\u{ff01}' => '!',
        '\u{ff08}' => '(',
        '\u{ff09}' => ')',
        '\u{ff0c}' => ',',
        '\u{ff0e}' => '.',
        '\u{ff1a}' => ':',
        '\u{ff1b}' => ';',
        '\u{ff1f}' => '?',
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201c}' | '\u{201d}' => '"',
        '\u{ff10}'..='\u{ff19}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        '\u{ff21}'..='\u{ff3a}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        '\u{ff41}'..='\u{ff5a}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        _ => ch,
    }
}

fn sentence_segments(text: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if is_sentence_boundary(ch) {
            let end = index + ch.len_utf8();
            segments.push(text[start..end].trim());
            start = end;
        }
    }
    if start < text.len() {
        segments.push(text[start..].trim());
    }
    segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn split_text_by_token_budget<F, E>(
    text: &str,
    max_tokens: usize,
    encode: &mut F,
) -> Result<Vec<String>, E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    let mut remaining = text.trim();
    let mut pieces = Vec::new();
    while !remaining.is_empty() {
        if encode(remaining)?.len() <= max_tokens {
            pieces.push(remaining.to_string());
            break;
        }

        let mut best_prefix_length = 0usize;
        for (index, _) in remaining.char_indices().skip(1) {
            let candidate = remaining[..index].trim();
            if !candidate.is_empty() && encode(candidate)?.len() <= max_tokens {
                best_prefix_length = index;
            } else if best_prefix_length > 0 {
                break;
            }
        }
        if best_prefix_length == 0 {
            best_prefix_length = remaining
                .char_indices()
                .nth(1)
                .map(|(index, _)| index)
                .unwrap_or(remaining.len());
        }

        let prefix = &remaining[..best_prefix_length];
        let scan_min = prefix.len().saturating_sub(25);
        let mut cut_index = best_prefix_length;
        for (index, ch) in prefix.char_indices().rev() {
            if index < scan_min {
                break;
            }
            if ch == ' ' || is_soft_boundary(ch) || is_sentence_boundary(ch) {
                cut_index = index + ch.len_utf8();
                break;
            }
        }

        let piece = remaining[..cut_index].trim();
        if !piece.is_empty() {
            pieces.push(piece.to_string());
        }
        remaining = remaining[cut_index..].trim();
    }
    Ok(pieces)
}

fn split_oversized_sentence<F, E>(
    sentence: &str,
    max_tokens: usize,
    encode: &mut F,
    chunks: &mut Vec<PreparedTextChunk>,
) -> Result<(), E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    for segment in soft_segments(sentence) {
        if encode(segment)?.len() <= max_tokens {
            push_encoded(chunks, encode, segment)?;
        } else {
            for piece in split_text_by_token_budget(segment, max_tokens, encode)? {
                push_encoded(chunks, encode, &piece)?;
            }
        }
    }
    Ok(())
}

fn soft_segments(text: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if is_soft_boundary(ch) || is_sentence_boundary(ch) {
            let end = index + ch.len_utf8();
            segments.push(text[start..end].trim());
            start = end;
        }
    }
    if start < text.len() {
        segments.push(text[start..].trim());
    }
    segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn push_encoded<F, E>(
    chunks: &mut Vec<PreparedTextChunk>,
    encode: &mut F,
    text: &str,
) -> Result<(), E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let token_ids = encode(text)?;
    if token_ids.is_empty() {
        return Ok(());
    }
    chunks.push(PreparedTextChunk {
        text: text.to_string(),
        token_ids,
    });
    Ok(())
}

fn push_preencoded(chunks: &mut Vec<PreparedTextChunk>, text: &str, token_ids: Vec<i64>) {
    let text = text.trim();
    if text.is_empty() || token_ids.is_empty() {
        return;
    }
    chunks.push(PreparedTextChunk {
        text: text.to_string(),
        token_ids,
    });
}

fn merge_prepared_chunks<F, E>(
    slices: Vec<PreparedTextChunk>,
    max_tokens: usize,
    encode: &mut F,
) -> Result<Vec<PreparedTextChunk>, E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_tokens = Vec::new();

    for slice in slices {
        if current.is_empty() {
            current = slice.text;
            current_tokens = slice.token_ids;
            continue;
        }
        let candidate = join_segment(&current, &slice.text);
        let candidate_tokens = encode(&candidate)?;
        if candidate_tokens.len() > max_tokens {
            chunks.push(PreparedTextChunk {
                text: current,
                token_ids: current_tokens,
            });
            current = slice.text;
            current_tokens = slice.token_ids;
        } else {
            current = candidate;
            current_tokens = candidate_tokens;
        }
    }

    if !current.is_empty() && !current_tokens.is_empty() {
        chunks.push(PreparedTextChunk {
            text: current,
            token_ids: current_tokens,
        });
    }
    Ok(chunks)
}

fn join_segment(current: &str, segment: &str) -> String {
    let segment = segment.trim();
    if current.is_empty() {
        segment.to_string()
    } else {
        format!("{current} {segment}")
    }
}

fn is_sentence_boundary(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}')
}

fn is_soft_boundary(ch: char) -> bool {
    matches!(
        ch,
        ',' | ';' | ':' | '\n' | '\r' | '\u{ff0c}' | '\u{3001}' | '\u{ff1b}' | '\u{ff1a}'
    )
}

fn is_terminal_sentence_punctuation(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}')
}

fn is_tight_punctuation(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | ',' | ';' | ':' | ')' | '"' | '\'')
}

fn is_opening_punctuation(ch: char) -> bool {
    matches!(ch, '(' | '"' | '\'')
}

fn is_ascii_alnum(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

fn has_speakable_content(text: &str) -> bool {
    text.chars().any(|ch| ch.is_alphanumeric() || is_cjk(ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_tokens(text: &str) -> Result<Vec<i64>, String> {
        Ok(text.chars().map(|ch| ch as i64).collect())
    }

    #[test]
    fn normalizes_whitespace_punctuation_and_mixed_text() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(
            prep.normalize("  你好   World１２３ ，  ready？\nOK  "),
            "你好 World123, ready?。OK。"
        );
    }

    #[test]
    fn appends_terminal_sentence_punctuation_when_missing() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(prep.normalize("你好"), "你好。");
        assert_eq!(prep.normalize("你好。"), "你好。");
        assert_eq!(prep.normalize("hello?"), " Hello?");
        assert_eq!(prep.normalize("暂停，"), "暂停,。");
    }

    #[test]
    fn normalizes_english_like_official_runtime() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(prep.normalize("hello world"), " Hello world.");
        assert_eq!(
            prep.normalize("this is a longer english sentence"),
            "This is a longer english sentence."
        );
    }

    #[test]
    fn guards_zh_hyphens_before_text_normalization() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(
            prep.normalize("语音-编码 版本-2 -3"),
            "语音,编码 版本 -2 -3。"
        );
    }

    #[test]
    fn merges_short_sentences_up_to_token_budget() {
        let prep = MossTextPreprocessor::new(100);

        let chunks = prep
            .prepare("第一句很短。 第二句也短。 third sentence.", char_tokens)
            .unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一句很短。 第二句也短。 third sentence."]
        );
    }

    #[test]
    fn merges_sentence_boundaries_inside_token_budget() {
        let prep = MossTextPreprocessor::new(100);

        let chunks = prep
            .prepare("第一段,包含逗号:但仍然是一句。第二句。", char_tokens)
            .unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一段,包含逗号:但仍然是一句。 第二句。"]
        );
    }

    #[test]
    fn splits_oversized_sentence_on_soft_boundaries() {
        let prep = MossTextPreprocessor::new(8);

        let chunks = prep.prepare("第一段,第二段,第三段。", char_tokens).unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一段,", "第二段,", "第三段。"]
        );
    }

    #[test]
    fn splits_oversized_segment_and_skips_empty_chunks() {
        let prep = MossTextPreprocessor::new(3);

        let chunks = prep.prepare("     abcdef     ", char_tokens).unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Abc", "def", "."]
        );
    }

    #[test]
    fn symbol_only_text_normalizes_to_empty() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(prep.normalize("```---!!!```"), "");
        assert!(prep
            .prepare(" -> --- !!! ", char_tokens)
            .unwrap()
            .is_empty());
    }
}
