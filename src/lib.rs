//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

// ---------- High level API ----------

/// Convert Markdown into Telegram MarkdownV2 and split into safe chunks.
pub fn transform(markdown: &str, max_len: usize) -> Vec<String> {
    let rendered = render_markdown(markdown);
    if rendered.is_empty() {
        return Vec::new();
    }

    let rendered = restore_blockquote_blank_lines(markdown, &rendered);

    if rendered.len() <= max_len {
        return vec![trim_chunk(&rendered)];
    }

    if let Some(chunks) = split_before_first_fence(&rendered, max_len) {
        return normalize_chunks(chunks);
    }

    if let Some(chunks) = split_simple_fenced_code(&rendered, max_len) {
        return normalize_chunks(chunks);
    }

    normalize_chunks(word_wrap_chunks(&rendered, max_len))
}

// ---------- Chunking ----------

fn split_before_first_fence(rendered: &str, max_len: usize) -> Option<Vec<String>> {
    let fence_pos = rendered.find("```")?;
    if fence_pos == 0 {
        return None;
    }

    let head = rendered[..fence_pos].trim_end();
    if head.is_empty() || head.len() > max_len {
        return None;
    }

    let tail = rendered[fence_pos..].to_string();

    let mut chunks = vec![head.to_string()];

    if tail.len() <= max_len {
        chunks.push(tail);
        return Some(chunks);
    }

    if let Some(mut tail_chunks) = split_simple_fenced_code(&tail, max_len) {
        chunks.append(&mut tail_chunks);
        return Some(chunks);
    }

    None
}

fn split_simple_fenced_code(rendered: &str, max_len: usize) -> Option<Vec<String>> {
    const FENCE_START: &str = "```\n";
    const FENCE_END: &str = "\n```";
    let fence_overhead = FENCE_START.len() + FENCE_END.len();

    let lines: Vec<&str> = rendered.lines().collect();
    if lines.len() < 2 || !lines.first()?.starts_with("```") || !lines.last()?.starts_with("```") {
        return None;
    }

    let body = &lines[1..lines.len().saturating_sub(1)];
    if body.is_empty() {
        return Some(vec![rendered.to_string()]);
    }

    let mut chunks = Vec::new();
    let available = max_len.saturating_sub(fence_overhead).max(1);

    for line in body {
        if line.len() + fence_overhead <= max_len {
            chunks.push(format!("{}{}{}", FENCE_START, line, FENCE_END));
        } else {
            for piece in split_long_word(line, available) {
                chunks.push(format!("{}{}{}", FENCE_START, piece, FENCE_END));
            }
        }
    }

    Some(chunks)
}

fn word_wrap_chunks(rendered: &str, max_len: usize) -> Vec<String> {
    if rendered.contains('\n') {
        return wrap_by_newlines(rendered, max_len);
    }

    wrap_by_tokens(rendered, max_len, tokenize_preserving_links)
}

fn wrap_by_newlines(rendered: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for seg in rendered.split_inclusive('\n') {
        if seg.len() > max_len {
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }
            for piece in split_long_word(seg, max_len) {
                chunks.push(piece);
            }
            continue;
        }

        let projected = current.len() + seg.len();
        if projected <= max_len {
            current.push_str(seg);
        } else {
            if !current.is_empty() {
                chunks.push(current);
            }
            current = seg.to_string();
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        vec![rendered.to_string()]
    } else {
        chunks
    }
}

fn wrap_by_tokens<F>(rendered: &str, max_len: usize, tokenizer: F) -> Vec<String>
where
    F: Fn(&str) -> Vec<String>,
{
    let mut chunks = Vec::new();
    let mut current = String::new();

    for token in tokenizer(rendered) {
        let space = if current.is_empty() { 0 } else { 1 };
        if current.len() + space + token.len() <= max_len {
            if space == 1 {
                current.push(' ');
            }
            current.push_str(&token);
            continue;
        }

        if !current.is_empty() {
            chunks.push(current);
            current = String::new();
        }

        if token.len() > max_len {
            for piece in split_long_word(&token, max_len) {
                chunks.push(piece);
            }
        } else {
            current = token;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        vec![rendered.to_string()]
    } else {
        chunks
    }
}

fn trim_chunk(s: &str) -> String {
    s.trim_end_matches('\n').to_string()
}

fn normalize_chunks(chunks: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = chunks.into_iter().map(|c| trim_chunk(&c)).collect();
    if out.len() > 1 {
        out.retain(|c| !c.trim().is_empty());
    }
    out
}

// ---------- Rendering ----------

fn render_markdown(input: &str) -> String {
    let parser = Parser::new_ext(input, Options::ENABLE_STRIKETHROUGH);

    let mut out = String::new();
    let mut list_blank_iter = compute_list_blank_lines(input).into_iter();
    let mut list_after_blank_iter = compute_list_after_blank_lines(input).into_iter();
    let mut link_stack: Vec<String> = Vec::new();

    let mut in_list_item = false;
    let mut in_blockquote = false;
    let mut has_content = false;
    let mut prev_was_heading = false;
    let mut prev_was_rule = false;
    let mut blockquote_pending_gap = false;

    // Preserve leading blank lines (pulldown_cmark skips them).
    let leading_blank = input.chars().take_while(|c| *c == '\n').count();
    for _ in 0..leading_blank {
        out.push('\n');
    }

    let push_newline = |out: &mut String, in_blockquote: bool| {
        out.push('\n');
        if in_blockquote {
            out.push('>');
        }
    };

    for event in parser {
        match event {
            Event::Start(tag) => {
                let mut gap_inserted = false;
                if blockquote_pending_gap {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push('\n');
                    blockquote_pending_gap = false;
                    gap_inserted = true;
                }
                match tag {
                    Tag::Paragraph => {
                        prev_was_heading = false;
                        if has_content && !gap_inserted && !in_list_item && !prev_was_rule {
                            push_newline(&mut out, in_blockquote);
                        }
                        prev_was_rule = false;
                    }
                    Tag::Heading { level, .. } => {
                        if has_content && !gap_inserted {
                            if prev_was_heading {
                                if !out.ends_with('\n') {
                                    push_newline(&mut out, in_blockquote);
                                }
                            } else if !out.ends_with("\n\n") {
                                push_newline(&mut out, in_blockquote);
                            }
                        }
                        out.push('*');
                        has_content = true;
                        prev_was_heading = true;
                        prev_was_rule = false;
                        match level {
                            HeadingLevel::H1 => out.push_str("⭐⭐ "),
                            HeadingLevel::H2 => out.push_str("⭐ "),
                            HeadingLevel::H3 => out.push_str("🔸🔸 "),
                            HeadingLevel::H4 => out.push_str("🔸 "),
                            HeadingLevel::H5 => out.push_str("▪️ "),
                            HeadingLevel::H6 => out.push_str("· "),
                        }
                    }
                    Tag::BlockQuote(_) => {
                        prev_was_heading = false;
                        prev_was_rule = false;
                        if has_content && !out.ends_with("\n\n") {
                            push_newline(&mut out, in_blockquote);
                        }
                        out.push('>');
                        in_blockquote = true;
                    }
                    Tag::Emphasis => {
                        out.push('_');
                        has_content = true;
                        prev_was_rule = false;
                    }
                    Tag::Strong => {
                        out.push('*');
                        has_content = true;
                        prev_was_rule = false;
                    }
                    Tag::Strikethrough => {
                        out.push('~');
                        has_content = true;
                        prev_was_rule = false;
                    }
                    Tag::Link { dest_url, .. } => {
                        link_stack.push(dest_url.to_string());
                        out.push('[');
                        has_content = true;
                        prev_was_rule = false;
                    }
                    Tag::List(_) => {
                        let blank_before = list_blank_iter.next().unwrap_or(false);
                        prev_was_heading = false;
                        prev_was_rule = false;
                        if has_content && !gap_inserted {
                            if in_blockquote {
                                if blank_before || (!out.ends_with('\n') && !out.ends_with('>')) {
                                    push_newline(&mut out, in_blockquote);
                                }
                            } else if blank_before || !out.ends_with('\n') {
                                push_newline(&mut out, in_blockquote);
                            }
                        }
                    }
                    Tag::Item => {
                        prev_was_heading = false;
                        prev_was_rule = false;
                        if has_content
                            && !gap_inserted
                            && !out.ends_with('\n')
                            && !(in_blockquote && out.ends_with('>'))
                        {
                            push_newline(&mut out, in_blockquote);
                        }
                        out.push('⦁');
                        out.push(' ');
                        has_content = true;
                        in_list_item = true;
                    }
                    Tag::CodeBlock(kind) => {
                        prev_was_heading = false;
                        prev_was_rule = false;
                        if has_content && !gap_inserted {
                            push_newline(&mut out, in_blockquote);
                        }
                        out.push_str("```");
                        if let CodeBlockKind::Fenced(lang) = kind {
                            if !lang.is_empty() {
                                out.push_str(lang.as_ref());
                            }
                        }
                        out.push('\n');
                        has_content = true;
                    }
                    _ => {}
                }
            }
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if !in_list_item {
                        push_newline(&mut out, in_blockquote);
                    }
                    prev_was_rule = false;
                }
                TagEnd::Heading(_) => {
                    out.push('*');
                    push_newline(&mut out, in_blockquote);
                    prev_was_rule = false;
                }
                TagEnd::Emphasis => out.push('_'),
                TagEnd::Strong => out.push('*'),
                TagEnd::Strikethrough => out.push('~'),
                TagEnd::Link => {
                    if let Some(dest) = link_stack.pop() {
                        out.push(']');
                        out.push('(');
                        out.push_str(&escape_url(&dest));
                        out.push(')');
                    }
                }
                TagEnd::Item => {
                    in_list_item = false;
                }
                TagEnd::List(_) => {
                    if let Some(blank_after) = list_after_blank_iter.next() {
                        if blank_after {
                            push_newline(&mut out, in_blockquote);
                        }
                    }
                }
                TagEnd::CodeBlock => {
                    if !out.ends_with('\n') {
                        push_newline(&mut out, in_blockquote);
                    }
                    out.push_str("```");
                    push_newline(&mut out, in_blockquote);
                    prev_was_rule = false;
                }
                TagEnd::BlockQuote(_) => {
                    if out.ends_with("\n>") {
                        out.truncate(out.len() - 2);
                    } else if out.ends_with('>') {
                        out.pop();
                    }
                    in_blockquote = false;
                    blockquote_pending_gap = true;
                }
                _ => {}
            },
            Event::Text(t) => {
                out.push_str(&escape_text(&t));
                if !t.is_empty() {
                    has_content = true;
                }
                prev_was_rule = false;
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&escape_text(&t));
                out.push('`');
                has_content = true;
                prev_was_rule = false;
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                out.push_str(&escape_text(&t));
                has_content = true;
                prev_was_rule = false;
            }
            Event::SoftBreak => push_newline(&mut out, in_blockquote),
            Event::HardBreak => {
                if in_list_item {
                    out.push_str("  ");
                    push_newline(&mut out, in_blockquote);
                    out.push_str("  ");
                } else if in_blockquote {
                    out.push_str("  ");
                    push_newline(&mut out, in_blockquote);
                } else {
                    push_newline(&mut out, in_blockquote);
                }
            }
            Event::Rule => {
                if out.ends_with('\n') {
                    out.push_str("\n————————\n\n");
                } else {
                    out.push_str("\n\n————————\n\n");
                }
                has_content = true;
                prev_was_heading = false;
                prev_was_rule = true;
            }
            _ => {}
        }
    }

    out.trim_end().to_string()
}

// ---------- Inline helpers ----------

/// Detect, in textual order, whether each list in the original markdown is
/// preceded by a blank line. We only care about the first item of each list,
/// and we ignore occurrences inside fenced code blocks.
fn compute_list_blank_lines(input: &str) -> Vec<bool> {
    let mut out = Vec::new();
    let mut prev_line_empty = false;
    let mut in_code_block = false;
    let mut in_list = false;

    for line in input.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            prev_line_empty = false;
            in_list = false;
            continue;
        }

        if in_code_block {
            prev_line_empty = trimmed.is_empty();
            continue;
        }

        let is_list_item = is_list_item_line(trimmed);

        if is_list_item {
            if !in_list {
                out.push(prev_line_empty);
                in_list = true;
            }
        } else if !trimmed.is_empty() {
            in_list = false;
        }

        prev_line_empty = trimmed.is_empty();
    }

    out
}

/// Detect whether each list in the original markdown is followed by at least
/// one blank line before the next non-empty line. Order matches markdown order
/// of list occurrences and ignores fenced code blocks.
fn compute_list_after_blank_lines(input: &str) -> Vec<bool> {
    let mut out = Vec::new();
    let mut in_code_block = false;
    let mut in_list = false;

    for line in input.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if in_list {
                out.push(false);
                in_list = false;
            }
            continue;
        }

        if in_code_block {
            continue;
        }

        let is_list_item = is_list_item_line(trimmed);

        if is_list_item {
            in_list = true;
            continue;
        }

        if in_list {
            // Current line is not part of the list, so the list just ended.
            out.push(trimmed.is_empty());
            in_list = false;

            // If this line is blank, keep scanning until a non-blank line so
            // we don't treat successive blank lines as separate list endings.
            if trimmed.is_empty() {
                continue;
            }
        }
    }

    if in_list {
        out.push(false);
    }

    out
}

fn is_list_item_line(trimmed: &str) -> bool {
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }

    let mut chars = trimmed.chars().peekable();
    let mut seen_digit = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            seen_digit = true;
            continue;
        }
        if c == '.' && seen_digit && chars.peek().map_or(false, |next| *next == ' ') {
            return true;
        }
        break;
    }

    false
}

const SPECIALS: [char; 19] = [
    '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!', '\\',
];

fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if SPECIALS.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn escape_url(s: &str) -> String {
    // Telegram only needs parentheses escaped in URLs when used inside markdown syntax.
    s.replace(')', "\\)").replace('(', "\\(")
}

fn tokenize_preserving_links(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Try to capture escaped markdown link `\[...](...)` as single token.
        if bytes[i] == b'\\' && i + 1 < len && bytes[i + 1] == b'[' {
            if let Some(bracket_rel) = s[i + 2..].find("\\]") {
                let end_bracket = i + 2 + bracket_rel; // start of "\\]"
                let after_bracket = end_bracket + 2;
                if after_bracket + 1 < len && s[after_bracket..].starts_with("\\(") {
                    let end = if let Some(paren_rel) = s[after_bracket + 2..].find("\\)") {
                        let end_paren = after_bracket + 2 + paren_rel;
                        (end_paren + 2).min(len)
                    } else {
                        len
                    };
                    let mut token = s[i..end].to_string();
                    token.retain(|c| c != '\\');
                    tokens.push(token);
                    i = end;
                    continue;
                }
            }
        }

        // Unescaped markdown link `[...](...)`
        if bytes[i] == b'[' && i + 1 < len {
            if let Some(bracket_rel) = s[i + 1..].find(']') {
                let end_bracket = i + 1 + bracket_rel;
                if end_bracket + 1 < len && bytes[end_bracket + 1] == b'(' {
                    if let Some(paren_rel) = s[end_bracket + 2..].find(')') {
                        let end_paren = end_bracket + 2 + paren_rel + 1;
                        let end = end_paren.min(len);
                        tokens.push(s[i..end].to_string());
                        i = end;
                        continue;
                    }
                }
            }
        }

        // Fallback: read until next whitespace.
        let start = i;
        while i < len && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        tokens.push(s[start..i].to_string());
    }

    tokens
}

fn split_long_word(word: &str, max_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();

    for ch in word.chars() {
        if buf.len() + ch.len_utf8() > max_len {
            out.push(buf);
            buf = String::new();
        }
        buf.push(ch);
    }

    if !buf.is_empty() {
        out.push(buf);
    }

    out
}

fn restore_blockquote_blank_lines(original: &str, rendered: &str) -> String {
    let rendered_lines: Vec<&str> = rendered.lines().collect();
    let mut idx = 0;
    let mut out = Vec::new();

    for line in original.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('>') && trimmed.trim() == ">" {
            // Preserve explicit blank lines inside blockquotes from the source.
            if let Some(r) = rendered_lines.get(idx) {
                if r.trim() == ">" {
                    idx += 1; // consume matching blank that renderer may have produced
                }
            }
            out.push(">".to_string());
        } else if let Some(r) = rendered_lines.get(idx) {
            out.push((*r).to_string());
            idx += 1;
        }
    }

    // Append any remaining rendered lines (e.g., renderer inserted extra blanks).
    for r in rendered_lines.iter().skip(idx) {
        out.push((*r).to_string());
    }

    out.join("\n")
}
