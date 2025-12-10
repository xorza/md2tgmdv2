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
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    prev_was_heading = false;
                    if has_content && !in_list_item && !(in_blockquote && out.ends_with("**>")) {
                        push_newline(&mut out, in_blockquote);
                    }
                }
                Tag::Heading { level, .. } => {
                    if has_content {
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
                    out.push_str("**>");
                    has_content = true;
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
                    if has_content {
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
                    if has_content && !out.ends_with('\n') && !(in_blockquote && out.ends_with('>'))
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
                    if has_content {
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
            },
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
                    // If we ended up with a dangling quote marker for the next line,
                    // drop it so the blockquote closes cleanly.
                    if out.ends_with('>') {
                        out.pop();
                        if out.ends_with('\n') {
                            out.pop();
                        }
                    }
                    out.push_str("||");
                    in_blockquote = false;
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
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut last_was_empty = false;
    let mut seen_nonempty = false;

    for line in input.lines() {
        // Preserve empty lines (including leading ones).
        if line.is_empty() {
            if current.len() + 1 > max_len && !current.is_empty() {
                blocks.push(current);
                current = String::new();
            }
            current.push('\n');
            last_was_empty = true;
            continue;
        }

        // If a single line is longer than the limit, flush current and hard-split the line.
        if line.len() > max_len {
            if !current.is_empty() {
                blocks.push(current);
                current = String::new();
            }
            blocks.extend(split_long_line(line, max_len));
            last_was_empty = false;
            continue;
        }

        let projected = if current.is_empty() {
            line.len()
        } else {
            current.len() + 1 + line.len()
        };

        if projected > max_len && !current.is_empty() {
            blocks.push(current);
            current = String::new();
        }

        if !current.is_empty() && !(last_was_empty && !seen_nonempty) {
            current.push('\n');
        }
        current.push_str(line);
        last_was_empty = false;
        seen_nonempty = true;
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
