use crate::robust::normalize_robust_text;

const DEFAULT_MAX_TOKENS_PER_CHUNK: usize = 180;

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
        Ok(chunks)
    }
}

fn normalize_tts_text(text: &str) -> String {
    let text = normalize_robust_text(text);
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
    let output = ensure_terminal_sentence_punctuation(output);
    if has_speakable_content(&output) {
        output
    } else {
        String::new()
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

fn split_oversized_sentence<F, E>(
    sentence: &str,
    max_tokens: usize,
    encode: &mut F,
    chunks: &mut Vec<PreparedTextChunk>,
) -> Result<(), E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    let mut current = String::new();
    for segment in soft_segments(sentence) {
        let candidate = join_segment(&current, segment);
        if !candidate.trim().is_empty() && encode(&candidate)?.len() <= max_tokens {
            current = candidate;
            continue;
        }

        push_encoded(chunks, encode, &current)?;
        current.clear();

        let segment_tokens = encode(segment)?;
        if segment_tokens.len() <= max_tokens {
            current.push_str(segment.trim());
        } else {
            split_oversized_segment(segment, max_tokens, encode, chunks)?;
        }
    }
    push_encoded(chunks, encode, &current)
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

fn split_oversized_segment<F, E>(
    segment: &str,
    max_tokens: usize,
    encode: &mut F,
    chunks: &mut Vec<PreparedTextChunk>,
) -> Result<(), E>
where
    F: FnMut(&str) -> Result<Vec<i64>, E>,
{
    let mut current = String::new();
    for ch in segment.chars() {
        let candidate = format!("{current}{ch}");
        if !current.is_empty() && encode(&candidate)?.len() > max_tokens {
            push_encoded(chunks, encode, &current)?;
            current.clear();
        }
        current.push(ch);
    }
    push_encoded(chunks, encode, &current)
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

fn join_segment(current: &str, segment: &str) -> String {
    let segment = segment.trim();
    if current.is_empty() {
        segment.to_string()
    } else {
        format!("{current} {segment}")
    }
}

fn is_sentence_boundary(ch: char) -> bool {
    matches!(
        ch,
        '.' | '!' | '?' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}'
    )
}

fn is_soft_boundary(ch: char) -> bool {
    matches!(
        ch,
        ',' | ';' | ':' | '\n' | '\r' | '\u{ff0c}' | '\u{3001}' | '\u{ff1b}' | '\u{ff1a}'
    )
}

fn ensure_terminal_sentence_punctuation(mut text: String) -> String {
    if text.is_empty() || text.ends_with(is_terminal_sentence_punctuation) {
        return text;
    }
    text.push('\u{3002}');
    text
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
        assert_eq!(prep.normalize("hello?"), "hello?");
        assert_eq!(prep.normalize("暂停，"), "暂停,。");
    }

    #[test]
    fn chunks_one_sentence_per_synthesis_chunk() {
        let prep = MossTextPreprocessor::new(100);

        let chunks = prep
            .prepare("第一句很短。 第二句也短。 third sentence.", char_tokens)
            .unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一句很短。", "第二句也短。", "third sentence."]
        );
    }

    #[test]
    fn keeps_soft_boundaries_inside_normal_length_sentence() {
        let prep = MossTextPreprocessor::new(100);

        let chunks = prep
            .prepare("第一段,包含逗号:但仍然是一句。第二句。", char_tokens)
            .unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec!["第一段,包含逗号:但仍然是一句。", "第二句。"]
        );
    }

    #[test]
    fn splits_oversized_sentence_on_soft_boundaries() {
        let prep = MossTextPreprocessor::new(8);

        let chunks = prep
            .prepare("第一段,第二段,第三段。", char_tokens)
            .unwrap();

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
            vec!["abc", "def", "\u{3002}"]
        );
    }

    #[test]
    fn symbol_only_text_normalizes_to_empty() {
        let prep = MossTextPreprocessor::default();

        assert_eq!(prep.normalize("```---!!!```"), "");
        assert!(prep.prepare(" -> --- !!! ", char_tokens).unwrap().is_empty());
    }
}
