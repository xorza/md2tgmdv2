//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! The public entry point is [`transform`]: feed it any Markdown string
//! and it returns one or more message-ready chunks (Telegram hard‑limit is 4096
//! chars, so we split conservatively by lines).

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

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

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if !out.is_empty() && !in_list_item {
                        out.push('\n');
                    }
                }
                Tag::Heading { level, .. } => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push('*');
                    if let pulldown_cmark::HeadingLevel::H2 = level {
                        out.push_str("✏ ");
                    }
                }
                Tag::Emphasis => out.push('_'),
                Tag::Strong => out.push('*'),
                Tag::Strikethrough => out.push('~'),
                Tag::Link { dest_url, .. } => {
                    link_stack.push(dest_url.to_string());
                    out.push('[');
                }
                Tag::List(_) => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                }
                Tag::Item => {
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                    out.push('⦁');
                    out.push(' ');
                    in_list_item = true;
                }
                Tag::CodeBlock(kind) => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str("```");
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
                        out.push('\n');
                    }
                }
                TagEnd::Heading(_) => {
                    out.push('*');
                    out.push('\n');
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
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    in_list_item = false;
                }
                TagEnd::List(_) => {}
                TagEnd::CodeBlock => {
                    out.push('\n');
                    out.push_str("```");
                    out.push('\n');
                    in_code_block = false;
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_code_block {
                    out.push_str(&t);
                } else {
                    out.push_str(&escape_text(&t));
                }
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&escape_text(&t));
                out.push('`');
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => {
                if in_list_item {
                    out.push_str("  \n  ");
                } else {
                    out.push('\n');
                }
            }
            Event::Rule => out.push_str("\n————————\n"),
            _ => {}
        }
    }

    out.trim().to_string()
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

    for line in input.lines() {
        // If a single line is longer than the limit, flush current and hard-split the line.
        if line.len() > max_len {
            if !current.is_empty() {
                blocks.push(current);
                current = String::new();
            }
            blocks.extend(split_long_line(line, max_len));
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

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_chars() {
        let out = transform("a*b_c[link](url)", TG_MAX_LEN);
        assert_eq!(out[0], "a\\*b\\_c[link](url)");
    }

    #[test]
    fn renders_heading_and_paragraph() {
        let md = "# Title\n\nHello *world*";
        let out = transform(md, TG_MAX_LEN);
        assert_eq!(out[0], "*Title*\n\nHello _world_");
    }

    #[test]
    fn splits_long_messages() {
        let md = "line one\n\nline two\n\nline three";
        let out = transform(md, 10);
        assert_eq!(out.len(), 3);
    }
}
