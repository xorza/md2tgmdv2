//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! The public entry point is [`transform`]: feed it any Markdown string
//! and it returns one or more message-ready chunks (Telegram hard‑limit is 4096
//! chars, so we split conservatively by lines).

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

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

fn is_list_item_line(trimmed: &str) -> bool {
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }

    // Ordered list marker like "1. "
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

/// Convert Markdown into Telegram MarkdownV2 and split into safe chunks.
pub fn transform(markdown: &str, max_len: usize) -> Vec<String> {
    let rendered = render_markdown(markdown);
    if rendered.is_empty() {
        return Vec::new();
    }
    let effective_max = if max_len > 1000 {
        max_len.saturating_sub(170)
    } else {
        max_len
    };
    let chunks = split_chunks(&rendered, effective_max);
    chunks
        .into_iter()
        .map(|chunk| strip_trailing_spaces(&chunk))
        .collect()
}

/// Render Markdown into Telegram-safe MarkdownV2 text.
fn render_markdown(input: &str) -> String {
    let parser = Parser::new_ext(input, Options::ENABLE_STRIKETHROUGH);

    let mut out = String::new();
    let mut list_blank_iter = compute_list_blank_lines(input).into_iter();
    let mut link_stack: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut in_list_item = false;
    let mut in_blockquote = false;
    let mut has_content = false;
    let mut prev_was_heading = false;
    let mut blockquote_pending_gap = false;

    // Preserve leading blank lines (pulldown_cmark skips them).
    let leading_blank = input.chars().take_while(|c| *c == '\n').count();
    for _ in 0..leading_blank {
        out.push('\n');
    }

    // Insert a newline, and when inside a blockquote prefix the new line
    // with the Telegram quote marker.
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
                        if has_content && !gap_inserted && !in_list_item {
                            push_newline(&mut out, in_blockquote);
                        }
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
                        match level {
                            HeadingLevel::H1 => out.push_str("📌 "),
                            HeadingLevel::H2 => out.push_str("✏ "),
                            HeadingLevel::H3 => out.push_str("📚 "),
                            HeadingLevel::H4 => out.push_str("🔖 "),
                            _ => {}
                        }
                    }
                    Tag::BlockQuote(_) => {
                        prev_was_heading = false;
                        if has_content && !out.ends_with("\n\n") {
                            push_newline(&mut out, in_blockquote);
                        }
                        out.push('>');
                        in_blockquote = true;
                    }
                    Tag::Emphasis => {
                        out.push('_');
                        has_content = true;
                    }
                    Tag::Strong => {
                        out.push('*');
                        has_content = true;
                    }
                    Tag::Strikethrough => {
                        out.push('~');
                        has_content = true;
                    }
                    Tag::Link { dest_url, .. } => {
                        link_stack.push(dest_url.to_string());
                        out.push('[');
                        has_content = true;
                    }
                    Tag::List(_) => {
                        let blank_before = list_blank_iter.next().unwrap_or(false);
                        prev_was_heading = false;
                        if has_content && !gap_inserted {
                            if in_blockquote {
                                if blank_before || (!out.ends_with('\n') && !out.ends_with('>')) {
                                    push_newline(&mut out, in_blockquote);
                                }
                            } else {
                                if blank_before || !out.ends_with('\n') {
                                    push_newline(&mut out, in_blockquote);
                                }
                            }
                        }
                    }
                    Tag::Item => {
                        prev_was_heading = false;
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
                        if has_content && !gap_inserted {
                            push_newline(&mut out, in_blockquote);
                        }
                        out.push_str("```");
                        has_content = true;
                        if let CodeBlockKind::Fenced(lang) = kind {
                            if !lang.is_empty() {
                                out.push_str(lang.as_ref());
                            }
                        }
                        out.push('\n');
                        in_code_block = true;
                    }
                    _ => {}
                }
            }
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if !in_list_item {
                        push_newline(&mut out, in_blockquote);
                    }
                }
                TagEnd::Heading(_) => {
                    out.push('*');
                    push_newline(&mut out, in_blockquote);
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
                TagEnd::List(_) => {}
                TagEnd::CodeBlock => {
                    if !out.ends_with('\n') {
                        push_newline(&mut out, in_blockquote);
                    }
                    out.push_str("```");
                    push_newline(&mut out, in_blockquote);
                    in_code_block = false;
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
                if in_code_block {
                    out.push_str(&escape_text(&t));
                } else {
                    out.push_str(&escape_text(&t));
                }
                if !t.is_empty() {
                    has_content = true;
                }
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&escape_text(&t));
                out.push('`');
                has_content = true;
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => {
                if in_list_item {
                    out.push_str("  ");
                    push_newline(&mut out, in_blockquote);
                    out.push_str("  ");
                } else {
                    if in_blockquote {
                        out.push_str("  ");
                        push_newline(&mut out, in_blockquote);
                    } else {
                        push_newline(&mut out, in_blockquote);
                    }
                }
            }
            Event::Rule => {
                if out.ends_with('\n') {
                    out.push_str("\n————————\n\n");
                } else {
                    out.push_str("\n\n————————\n\n");
                }
                has_content = true;
            }
            _ => {}
        }
    }

    out.trim_end().to_string()
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

/// Remove trailing spaces that appear because Markdown uses two spaces for soft line breaks.
/// We keep code fences untouched to avoid altering literal content.
fn strip_trailing_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_code_block = false;

    for line in input.lines() {
        let trimmed_for_fence = line.trim_start_matches('>');
        let is_fence = trimmed_for_fence.starts_with("```");

        if is_fence {
            in_code_block = !in_code_block;
            out.push_str(line);
        } else if in_code_block {
            // Inside code blocks, preserve whitespace exactly.
            out.push_str(line);
        } else {
            out.push_str(line.trim_end());
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

fn split_chunks(input: &str, max_len: usize) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_code_lang: Option<String> = None;
    let mut continuing_code_chunk = false;
    let mut prev_line_empty = false;

    let push_line = |buf: &mut String, line: &str| {
        if line.is_empty() {
            buf.push('\n');
            return;
        }
        if buf.is_empty() {
            buf.push_str(line);
            return;
        }
        let only_newlines = buf.chars().all(|c| c == '\n');
        if only_newlines {
            buf.push_str(line);
        } else {
            buf.push('\n');
            buf.push_str(line);
        }
    };

    for line in input.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```");
        let fence_lang = if is_fence {
            trimmed.trim_start_matches("```").to_string()
        } else {
            String::new()
        };

        // Determine if we need to start a new chunk.
        let projected = if current.is_empty() {
            line.len()
        } else {
            current.len() + 1 + line.len()
        };
        let projected_with_fence = if in_code_lang.is_some() {
            // If we split while inside a code block, we'll need to append a closing fence.
            projected + 4 // "```" + possible newline
        } else {
            projected
        };

        if projected_with_fence > max_len && !current.is_empty() {
            // Push current chunk.
            if in_code_lang.is_some() {
                if !current.ends_with('\n') {
                    current.push('\n');
                }
                current.push_str("```");
            }
            blocks.push(current);
            current = String::new();

            // If we are splitting in the middle of a code block, seed the next chunk with the opening fence.
            if in_code_lang.is_some() {
                let fence = format!("```{}", in_code_lang.as_deref().unwrap_or(""));
                current.push_str(&fence);
                continuing_code_chunk = true;
            }
            prev_line_empty = false;
        }

        // If a single line is still too long, hard split it (outside of code block handling).
        if line.len() > max_len {
            let pieces = split_long_line(line, max_len);
            for (i, piece) in pieces.iter().enumerate() {
                if i > 0 {
                    blocks.push(current);
                    current = String::new();
                    if in_code_lang.is_some() && !is_fence {
                        let fence = format!("```{}", in_code_lang.as_deref().unwrap_or(""));
                        current.push_str(&fence);
                    }
                }
                push_line(&mut current, piece);
            }
            continue;
        }

        let mut line_to_push = line;

        if in_code_lang.is_some() && continuing_code_chunk && prev_line_empty && !line.is_empty() {
            line_to_push = line.trim_start();
        }

        // If we are closing a continued code block, ensure there's a blank line before the closing fence.
        if is_fence && in_code_lang.is_some() && continuing_code_chunk && !prev_line_empty {
            push_line(&mut current, "");
        }

        push_line(&mut current, line_to_push);

        // Toggle code fence state *after* adding the line, so we seed the next chunk correctly.
        if is_fence {
            if in_code_lang.is_some() {
                in_code_lang = None;
                continuing_code_chunk = false;
            } else {
                in_code_lang = Some(fence_lang);
            }
        }

        prev_line_empty = line.is_empty();
    }

    if !current.is_empty() {
        blocks.push(current);
    }

    blocks
}

fn split_long_line(line: &str, max_len: usize) -> Vec<String> {
    // First, try to split on word boundaries to avoid leading/trailing spaces.
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        let word_len = word.len();
        let extra_space = if current.is_empty() { 0 } else { 1 };

        if word_len + extra_space > max_len {
            // Word itself too long — fall back to char-level splitting.
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }
            chunks.extend(split_long_word(word, max_len));
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word_len <= max_len {
            current.push(' ');
            current.push_str(word);
        } else {
            chunks.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    if chunks.is_empty() {
        // All whitespace or empty — fall back to character splitting to preserve behavior.
        split_long_word(line, max_len)
    } else {
        chunks
    }
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
