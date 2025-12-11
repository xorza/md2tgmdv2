//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

#[derive(Debug)]
pub struct Converter {
    max_len: usize,
    buffer: String,
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            max_len: TELEGRAM_BOT_MAX_MESSAGE_LENGTH,
            buffer: String::new(),
        }
    }
}

impl Converter {
    pub fn new(max_len: usize) -> Self {
        Self {
            max_len,
            ..Default::default()
        }
    }

    /// Convert Markdown into Telegram MarkdownV2 and split into safe chunks.
    pub fn go(&self, markdown: &str) -> anyhow::Result<Vec<String>> {
        let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
        for event in parser {
            match event {
                Event::Start(tag) => {
                    self.start_tag(tag);
                }
                Event::End(tag) => {
                    self.end_tag(tag);
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

    fn start_tag(&self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                println!("Paragraph");
            }
            Tag::Heading { level, .. } => {
                println!("Heading");
            }
            Tag::BlockQuote(kind) => {
                println!("BlockQuote");
            }
            Tag::CodeBlock(_) => {
                println!("CodeBlock");
            }
            Tag::HtmlBlock => {
                println!("HtmlBlock");
            }
            Tag::List(number) => {
                println!("List");
            }
            Tag::Item => {
                println!("Item");
            }
            Tag::FootnoteDefinition(_) => {
                println!("FootnoteDefinition");
            }
            Tag::Table(_) => {
                println!("Table");
            }
            Tag::TableHead => {
                println!("TableHead");
            }
            Tag::TableRow => {
                println!("TableRow");
            }
            Tag::TableCell => {
                println!("TableCell");
            }
            Tag::Subscript => {
                println!("Subscript");
            }
            Tag::Superscript => {
                println!("Superscript");
            }
            Tag::Emphasis => {
                println!("Emphasis");
            }
            Tag::Strong => {
                println!("Strong");
            }
            Tag::Strikethrough => {
                println!("Strikethrough");
            }
            Tag::Link { .. } => {
                println!("Link");
            }
            Tag::Image { .. } => {
                println!("Image");
            }
            Tag::MetadataBlock(kind) => {
                println!("MetadataBlock");
            }
            Tag::DefinitionList => {
                println!("DefinitionList");
            }
            Tag::DefinitionListTitle => {
                println!("DefinitionListTitle");
            }
            Tag::DefinitionListDefinition => {
                println!("DefinitionListDefinition");
            }
        }
    }

    fn end_tag(&self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                println!("EndParagraph");
            }
            TagEnd::Heading(level) => {
                println!("EndHeading");
            }
            TagEnd::BlockQuote(kind) => {
                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                println!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                println!("EndHtmlBlock");
            }
            TagEnd::List(number) => {
                println!("EndList");
            }
            TagEnd::Item => {
                println!("EndItem");
            }
            TagEnd::FootnoteDefinition => {
                println!("EndFootnoteDefinition");
            }
            TagEnd::Table => {
                println!("EndTable");
            }
            TagEnd::TableHead => {
                println!("EndTableHead");
            }
            TagEnd::TableRow => {
                println!("EndTableRow");
            }
            TagEnd::TableCell => {
                println!("EndTableCell");
            }
            TagEnd::Subscript => {
                println!("EndSubscript");
            }
            TagEnd::Superscript => {
                println!("EndSuperscript");
            }
            TagEnd::Emphasis => {
                println!("EndEmphasis");
            }
            TagEnd::Strong => {
                println!("EndStrong");
            }
            TagEnd::Strikethrough => {
                println!("EndStrikethrough");
            }
            TagEnd::Link { .. } => {
                println!("EndLink");
            }
            TagEnd::Image { .. } => {
                println!("EndImage");
            }
            TagEnd::MetadataBlock(kind) => {
                println!("EndMetadataBlock");
            }
            TagEnd::DefinitionList => {
                println!("EndDefinitionList");
            }
            TagEnd::DefinitionListTitle => {
                println!("EndDefinitionListTitle");
            }
            TagEnd::DefinitionListDefinition => {
                println!("EndDefinitionListDefinition");
            }
        }
    }
}

// #[test]
// fn test() -> anyhow::Result<()> {
//     let text = include_str!("../tests/1-input.md");
//     let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

//     let text = include_str!("../tests/3-input.md");
//     let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

//     Ok(())
// }
