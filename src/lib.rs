//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

// ---------- High level API ----------

/// Convert Markdown into Telegram MarkdownV2 and split into safe chunks.
pub fn transform(markdown: &str, max_len: usize) -> anyhow::Result<Vec<String>> {
    let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
    for event in parser {
        match event {
            Event::Start(tag) => {
                start_tag(tag);
            }
            Event::End(tag) => {
                end_tag(tag);
            }
            Event::Text(txt) => {
                println!("{}", txt);
            }
            _ => return Err(anyhow::anyhow!("Unsupported event type")),
        }
    }

    Ok(vec![])
}

fn start_tag(tag: Tag) {
    match tag {
        Tag::Paragraph => {
            println!("Paragraph");
        }
        Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } => {
            println!("Heading {}", level);
        }
        Tag::BlockQuote(kind) => {
            println!("Block Quote");
        }
        Tag::CodeBlock(kind) => {
            println!("Code Block");
        }
        Tag::HtmlBlock => {
            println!("HTML Block");
        }
        _ => {}
    }
}

fn end_tag(tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => {
            println!("Paragraph End");
        }
        TagEnd::Heading(level) => {
            // Handle heading events here
        }
        _ => {}
    }
}

#[test]
fn test() {
    let text = include_str!("../tests/1-input.md");
    let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

    let text = include_str!("../tests/3-input.md");
    let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
}
