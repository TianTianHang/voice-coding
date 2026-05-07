const PROTECTED_START: char = '\u{e000}';
const PROTECTED_END: char = '\u{e001}';

#[derive(Debug, Clone)]
struct ProtectedSpan {
    value: String,
}

pub(crate) fn normalize_robust_text(text: &str) -> String {
    let cleaned = remove_invisible_controls(text);
    let markdown = cleanup_markdown(&cleaned);
    let linked = simplify_markdown_links(&markdown);
    let inline_code = linked.replace('`', "");
    let (protected, spans) = protect_spans(&inline_code);
    let symbols = normalize_symbols(&protected);
    let restored = restore_spans(&symbols, &spans);
    collapse_spacing_and_punctuation(&restored)
}

fn remove_invisible_controls(text: &str) -> String {
    text.chars()
        .filter_map(|ch| match ch {
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' => None,
            '\n' | '\r' | '\t' => Some(ch),
            ch if ch.is_control() => None,
            ch => Some(normalize_full_width_char(ch)),
        })
        .collect()
}

fn normalize_full_width_char(ch: char) -> char {
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
        '\u{ff3b}' => '[',
        '\u{ff3d}' => ']',
        '\u{ff5b}' => '{',
        '\u{ff5d}' => '}',
        '\u{ff5c}' => '|',
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201c}' | '\u{201d}' => '"',
        '\u{ff10}'..='\u{ff19}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        '\u{ff21}'..='\u{ff3a}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        '\u{ff41}'..='\u{ff5a}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
        _ => ch,
    }
}

fn cleanup_markdown(text: &str) -> String {
    let mut output = Vec::new();
    let mut in_fence = false;

    for raw_line in text.lines() {
        let mut line = raw_line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if line.is_empty() {
            output.push(String::new());
            continue;
        }
        if is_table_separator(line) {
            continue;
        }

        line = strip_repeated_prefix(line, '>');
        line = strip_heading(line);
        line = strip_list_marker(line);
        line = strip_task_marker(line);

        let mut cleaned = String::with_capacity(line.len());
        for ch in line.chars() {
            match ch {
                '|' if !in_fence => cleaned.push(' '),
                '*' | '~' if !in_fence => cleaned.push(' '),
                _ => cleaned.push(ch),
            }
        }
        output.push(cleaned);
    }

    output.join("\n")
}

fn strip_repeated_prefix(mut line: &str, marker: char) -> &str {
    loop {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(marker) {
            line = rest.trim_start();
        } else {
            return trimmed;
        }
    }
}

fn strip_heading(line: &str) -> &str {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) {
        trimmed[hashes..].trim_start()
    } else {
        trimmed
    }
}

fn strip_list_marker(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return rest.trim_start();
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return rest.trim_start();
    }
    if let Some(rest) = trimmed.strip_prefix("+ ") {
        return rest.trim_start();
    }

    let mut chars = trimmed.char_indices();
    let mut number_end = None;
    for (index, ch) in &mut chars {
        if ch.is_ascii_digit() {
            continue;
        }
        if matches!(ch, '.' | ')') {
            number_end = Some(index + ch.len_utf8());
        }
        break;
    }
    if let Some(end) = number_end {
        return trimmed[end..].trim_start();
    }
    trimmed
}

fn strip_task_marker(line: &str) -> &str {
    let trimmed = line.trim_start();
    for marker in ["[ ]", "[x]", "[X]"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return rest.trim_start();
        }
    }
    trimmed
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|')
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '|' | '-' | ':' | ' ' | '\t'))
        && trimmed.chars().any(|ch| ch == '-')
}

fn simplify_markdown_links(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        output.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find(']') else {
            output.push_str(&rest[open..]);
            return output;
        };
        let label = &after_open[..close];
        let after_close = &after_open[close + 1..];
        if let Some(after_paren) = after_close.strip_prefix('(') {
            if let Some(end_paren) = after_paren.find(')') {
                output.push_str(label.trim());
                rest = &after_paren[end_paren + 1..];
                continue;
            }
        }
        output.push('[');
        output.push_str(label);
        output.push(']');
        rest = after_close;
    }
    output.push_str(rest);
    output
}

fn protect_spans(text: &str) -> (String, Vec<ProtectedSpan>) {
    let mut output = String::with_capacity(text.len());
    let mut spans = Vec::new();

    for token in split_keep_whitespace(text) {
        if token.chars().all(char::is_whitespace) {
            output.push_str(token);
            continue;
        }

        let (prefix, core, suffix) = trim_token_edges(token);
        if !core.is_empty() && is_protected_core(core) {
            output.push_str(prefix);
            let index = spans.len();
            spans.push(ProtectedSpan {
                value: readable_protected_span(core),
            });
            output.push(PROTECTED_START);
            output.push_str(&index.to_string());
            output.push(PROTECTED_END);
            output.push_str(suffix);
        } else {
            output.push_str(token);
        }
    }

    (output, spans)
}

fn split_keep_whitespace(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut last_space = None;
    for (index, ch) in text.char_indices() {
        let is_space = ch.is_whitespace();
        match last_space {
            None => last_space = Some(is_space),
            Some(previous) if previous != is_space => {
                parts.push(&text[start..index]);
                start = index;
                last_space = Some(is_space);
            }
            _ => {}
        }
    }
    if start < text.len() {
        parts.push(&text[start..]);
    }
    parts
}

fn trim_token_edges(token: &str) -> (&str, &str, &str) {
    let start = token
        .char_indices()
        .find(|(index, ch)| {
            !(matches!(ch, '"' | '\'' | '(' | '[' | '{' | '<') || *ch == '.' && *index > 0)
        })
        .map(|(index, _)| index)
        .unwrap_or(token.len());
    let end = token
        .char_indices()
        .rev()
        .find(|(_, ch)| !matches!(ch, '"' | '\'' | ')' | ']' | '}' | '>' | ',' | ';' | ':' | '!' | '?'))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(start);
    (&token[..start], &token[start..end], &token[end..])
}

fn is_protected_core(core: &str) -> bool {
    let lower = core.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("www.")
        || is_email(core)
        || is_mention_or_hashtag(core)
        || is_dot_version(core)
        || is_file_like(core)
        || is_short_identifier(core)
}

fn is_email(core: &str) -> bool {
    let Some((name, domain)) = core.split_once('@') else {
        return false;
    };
    !name.is_empty() && domain.contains('.') && domain.len() > 3
}

fn is_mention_or_hashtag(core: &str) -> bool {
    let Some(rest) = core.strip_prefix('@').or_else(|| core.strip_prefix('#')) else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn is_dot_version(core: &str) -> bool {
    let version = core.strip_prefix('v').or_else(|| core.strip_prefix('V')).unwrap_or(core);
    let mut parts = version.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    first.chars().all(|ch| ch.is_ascii_digit())
        && parts.clone().count() >= 1
        && parts.all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_file_like(core: &str) -> bool {
    if core.starts_with('.') && core[1..].chars().any(|ch| ch.is_ascii_alphanumeric()) {
        return true;
    }
    let has_separator = core.contains('/') || core.contains('\\');
    let has_extension = core
        .rsplit_once('.')
        .map(|(_, ext)| {
            (1..=8).contains(&ext.len())
                && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
        })
        .unwrap_or(false);
    has_separator || has_extension
}

fn is_short_identifier(core: &str) -> bool {
    core.len() <= 64
        && core.contains('_')
        && core
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn readable_protected_span(core: &str) -> String {
    if core.starts_with('@') || core.starts_with('#') {
        return core[1..].replace(['_', '-'], " ");
    }
    let readable = core.replace(['_', '/', '\\'], " ");
    if let Some(dot_file) = readable.strip_prefix('.') {
        return dot_file.to_string();
    }
    readable
}

fn normalize_symbols(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == PROTECTED_START {
            output.push(ch);
            for next in chars.by_ref() {
                output.push(next);
                if next == PROTECTED_END {
                    break;
                }
            }
            continue;
        }

        match ch {
            '-' | '\u{2013}' | '\u{2014}' | '\u{2015}' => {
                while matches!(chars.peek(), Some('-' | '\u{2013}' | '\u{2014}' | '\u{2015}' | '>')) {
                    chars.next();
                }
                output.push('。');
            }
            '=' if matches!(chars.peek(), Some('>')) => {
                chars.next();
                output.push('。');
            }
            '<' if matches!(chars.peek(), Some('-' | '=')) => {
                chars.next();
                output.push('。');
            }
            '_' | '/' | '\\' => output.push(' '),
            '[' | ']' | '{' | '}' | '<' | '>' => output.push(' '),
            '$' | '^' | '&' => output.push(' '),
            _ => output.push(ch),
        }
    }

    output
}

fn restore_spans(text: &str, spans: &[ProtectedSpan]) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != PROTECTED_START {
            output.push(ch);
            continue;
        }

        let mut index = String::new();
        for next in chars.by_ref() {
            if next == PROTECTED_END {
                break;
            }
            index.push(next);
        }
        if let Ok(index) = index.parse::<usize>() {
            if let Some(span) = spans.get(index) {
                output.push_str(&span.value);
            }
        }
    }

    output
}

fn collapse_spacing_and_punctuation(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut last_was_space = true;
    let mut last_punctuation: Option<char> = None;

    for ch in text.chars() {
        let normalized = match ch {
            '\n' | '\r' => '。',
            '\t' => ' ',
            '\u{3001}' | '\u{ff0c}' => ',',
            '\u{3002}' => '。',
            '\u{ff01}' => '!',
            '\u{ff1f}' => '?',
            _ => ch,
        };

        if normalized.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
            continue;
        }

        if is_collapsible_punctuation(normalized) {
            let stable = stable_punctuation(normalized);
            if last_punctuation == Some(stable) {
                continue;
            }
            if output.ends_with(' ') {
                output.pop();
            }
            output.push(stable);
            last_was_space = false;
            last_punctuation = Some(stable);
            continue;
        }

        output.push(normalized);
        last_was_space = false;
        last_punctuation = None;
    }

    output.trim().to_string()
}

fn is_collapsible_punctuation(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | ',' | ';' | ':' | '。' | '，' | '；' | '：')
}

fn stable_punctuation(ch: char) -> char {
    match ch {
        '，' => ',',
        '；' => ';',
        '：' => ':',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_markdown_and_keeps_natural_text() {
        let input = "# Done\n- 修改 `src-tauri/tts-moss/src/text.rs`\n> **注意** [文档](https://example.com)";

        assert_eq!(
            normalize_robust_text(input),
            "Done。修改 src-tauri tts-moss src text.rs。 注意 文档"
        );
    }

    #[test]
    fn protects_technical_spans_before_symbol_cleanup() {
        let input = ".env app.js.map v2.3.1 https://example.com/a_b foo_bar @team #release_notes";

        assert_eq!(
            normalize_robust_text(input),
            "env app.js.map v2.3.1 https: example.com a b foo bar team release notes"
        );
    }

    #[test]
    fn trim_token_edges_keeps_leading_and_internal_dots_in_core() {
        assert_eq!(trim_token_edges(".env"), ("", ".env", ""));
        assert_eq!(trim_token_edges("(app.js),"), ("(", "app.js", "),"));
    }

    #[test]
    fn collapses_symbol_heavy_text() {
        assert_eq!(
            normalize_robust_text("a -> b —— c？？！！\u{200b}\u{0007}"),
            "a。 b。 c?!"
        );
    }
}
