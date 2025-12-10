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
            Event::Code(txt) => {
                println!("{}", txt);
            }
            Event::InlineMath(txt) => {
                println!("{}", txt);
            }
            Event::DisplayMath(txt) => {
                println!("{}", txt);
            }
            Event::Html(txt) => {
                println!("{}", txt);
            }
            Event::InlineHtml(txt) => {
                println!("{}", txt);
            }
            Event::FootnoteReference(txt) => {
                println!("{}", txt);
            }
            Event::SoftBreak => {
                println!("SoftBreak");
            }
            Event::HardBreak => {
                println!("HardBreak");
            }
            Event::Rule => {
                println!("Rule");
            }
            Event::TaskListMarker(b) => {
                println!("TaskListMarker({})", b);
            }
        }
    }

    Ok(vec![])
}

fn start_tag(tag: Tag) {
    match tag {
        Tag::Paragraph => {}
        Tag::Heading { level, .. } => {}
        Tag::BlockQuote(kind) => {}
        Tag::CodeBlock(_) => {}
        Tag::HtmlBlock => {}
        Tag::List(number) => {}
        Tag::Item => {}
        Tag::FootnoteDefinition(_) => {}
        Tag::Table(_) => {}
        Tag::TableHead => {}
        Tag::TableRow => {}
        Tag::TableCell => {}
        Tag::Subscript => {}
        Tag::Superscript => {}
        Tag::Emphasis => {}
        Tag::Strong => {}
        Tag::Strikethrough => {}
        Tag::Link { .. } => {}
        Tag::Image { .. } => {}
        Tag::MetadataBlock(kind) => {}
        Tag::DefinitionList => {}
        Tag::DefinitionListTitle => {}
        Tag::DefinitionListDefinition => {}
    }
}

fn end_tag(tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => {}
        TagEnd::Heading(level) => {}
        TagEnd::BlockQuote(kind) => {}
        TagEnd::CodeBlock => {}
        TagEnd::HtmlBlock => {}
        TagEnd::List(number) => {}
        TagEnd::Item => {}
        TagEnd::FootnoteDefinition => {}
        TagEnd::Table => {}
        TagEnd::TableHead => {}
        TagEnd::TableRow => {}
        TagEnd::TableCell => {}
        TagEnd::Subscript => {}
        TagEnd::Superscript => {}
        TagEnd::Emphasis => {}
        TagEnd::Strong => {}
        TagEnd::Strikethrough => {}
        TagEnd::Link { .. } => {}
        TagEnd::Image { .. } => {}
        TagEnd::MetadataBlock(kind) => {}
        TagEnd::DefinitionList => {}
        TagEnd::DefinitionListTitle => {}
        TagEnd::DefinitionListDefinition => {}
    }
}

#[test]
fn test() -> anyhow::Result<()> {
    let text = include_str!("../tests/1-input.md");
    let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

    let text = include_str!("../tests/3-input.md");
    let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

    Ok(())
}
