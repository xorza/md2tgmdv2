//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! The public entry point is [`transform`]: feed it any Markdown string
//! and it returns one or more message-ready chunks (Telegram hard‑limit is 4096
//! chars, so we split conservatively by lines).

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TG_MAX_LEN: usize = 4096;

/// Convert Markdown into Telegram MarkdownV2 and split into safe chunks.
pub fn transform(markdown: &str, max_len: usize) -> Vec<String> {
    let rendered = render_markdown(markdown);
    if rendered.is_empty() {
        return Vec::new();
    }
    split_chunks(&rendered, max_len)
}

/// Render Markdown into Telegram-safe MarkdownV2 text.
fn render_markdown(input: &str) -> String {
    let parser = Parser::new_ext(input, Options::ENABLE_STRIKETHROUGH);

    let mut out = String::new();
    let mut link_stack: Vec<String> = Vec::new();
    let mut in_code_block = false;
    let mut in_list_item = false;
    let mut in_blockquote = false;
    let mut has_content = false;
    let mut prev_was_heading = false;
    let mut blockquote_start: Option<usize> = None;
    let mut blockquote_paragraphs: usize = 0;
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
                        if has_content
                            && !gap_inserted
                            && !in_list_item
                            && !(in_blockquote && blockquote_paragraphs == 0)
                        {
                            push_newline(&mut out, in_blockquote);
                        }
                        if in_blockquote {
                            blockquote_paragraphs += 1;
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
                        blockquote_start = Some(out.len());
                        blockquote_paragraphs = 0;
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
                        prev_was_heading = false;
                        if has_content && !gap_inserted {
                            if in_blockquote {
                                if !out.ends_with('\n') && !out.ends_with('>') {
                                    push_newline(&mut out, in_blockquote);
                                }
                            } else {
                                push_newline(&mut out, in_blockquote);
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
                    if let Some(start) = blockquote_start.take() {
                        if blockquote_paragraphs > 1 {
                            out.insert_str(start, "**");
                            out.push_str("||");
                        }
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

fn split_chunks(input: &str, max_len: usize) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_code_lang: Option<String> = None;
    let mut continuing_code_chunk = false;
    let mut prev_line_empty = false;

    let mut push_line = |buf: &mut String, line: &str| {
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

        if projected > max_len && !current.is_empty() {
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
            prev_line_empty = true;
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
    let mut out = Vec::new();
    let mut buf = String::new();

    for ch in line.chars() {
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
