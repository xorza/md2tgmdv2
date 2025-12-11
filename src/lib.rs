//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

use anyhow::anyhow;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

#[derive(Debug)]
pub struct Converter {
    max_len: usize,
    result: Vec<String>,
    buffer: String,
    stack: Vec<Descriptor>,
    prev_desc: Option<Descriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Descriptor {
    Paragraph,
    List,
    Item,
    Strong,
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            max_len: TELEGRAM_BOT_MAX_MESSAGE_LENGTH,
            result: vec!["".to_string()],
            buffer: String::new(),
            stack: Vec::new(),
            prev_desc: None,
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
    pub fn go(&mut self, markdown: &str) -> anyhow::Result<Vec<String>> {
        *self = Self::new(self.max_len);

        let markdown = markdown.trim();
        if markdown.is_empty() {
            return Ok(vec![]);
        }

        let parser = Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH);
        for event in parser {
            match event {
                Event::Start(tag) => {
                    self.start_tag(tag)?;
                }
                Event::End(tag) => {
                    self.end_tag(tag)?;
                }
                Event::Text(txt) => {
                    self.add_to_result(&txt, true);
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
                    self.add_to_result("\n", false);
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

        if !self.stack.is_empty() {
            return Err(anyhow!("Unbalanced tags"));
        }

        Ok(std::mem::take(&mut self.result))
    }

    fn add_to_result(&mut self, txt: &str, escape: bool) {
        if escape {
            let escaped = self.escape_text(&txt);
            self.result.last_mut().unwrap().push_str(&escaped);
        } else {
            self.result.last_mut().unwrap().push_str(txt);
        }
    }

    fn start_tag(&mut self, tag: Tag) -> anyhow::Result<()> {
        match tag {
            Tag::Paragraph => {
                if matches!(self.prev_desc, Some(Descriptor::Paragraph)) {
                    self.add_to_result("\n\n", false);
                } else if self.prev_desc.is_some() {
                    self.add_to_result("\n", false);
                }
                self.stack.push(Descriptor::Paragraph);
            }
            Tag::Heading { .. } => {
                println!("Heading");
            }
            Tag::BlockQuote(_kind) => {
                println!("BlockQuote");
            }
            Tag::CodeBlock(_) => {
                println!("CodeBlock");
            }
            Tag::HtmlBlock => {
                println!("HtmlBlock");
            }
            Tag::List(_) => {
                self.stack.push(Descriptor::List);
            }
            Tag::Item => {
                self.add_to_result("⦁ ", false);
                self.stack.push(Descriptor::Item);
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
                self.add_to_result("*", false);
                self.stack.push(Descriptor::Strong);
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
            Tag::MetadataBlock(_) => {
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

        self.prev_desc = None;

        Ok(())
    }

    fn end_tag(&mut self, tag: TagEnd) -> anyhow::Result<()> {
        match tag {
            TagEnd::Paragraph => {
                self.close_descriptor(Descriptor::Paragraph)?;
            }
            TagEnd::Heading(_) => {
                println!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                println!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                println!("EndHtmlBlock");
            }
            TagEnd::List(_) => {
                self.close_descriptor(Descriptor::List)?;
            }
            TagEnd::Item => {
                self.close_descriptor(Descriptor::Item)?;
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
                self.close_descriptor(Descriptor::Strong)?;
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
            TagEnd::MetadataBlock(_) => {
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

        Ok(())
    }

    fn get_last_descriptor(&self) -> Descriptor {
        self.stack.last().expect("Unexpected end tag").clone()
    }

    fn close_descriptor(&mut self, descriptor: Descriptor) -> anyhow::Result<()> {
        self.prev_desc = Some(descriptor);
        let last = self.stack.pop().expect("Unexpected end tag");

        assert_eq!(last, descriptor, "Unexpected end tag");

        match descriptor {
            Descriptor::Strong => {
                self.add_to_result("*", false);
            }
            _ => {}
        }

        Ok(())
    }

    fn escape_text(&self, text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('*', "\\*")
            .replace('_', "\\_")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('~', "\\~")
            .replace('`', "\\`")
            .replace('>', "\\>")
            .replace('#', "\\#")
            .replace('+', "\\+")
            .replace('-', "\\-")
            .replace('=', "\\=")
            .replace('|', "\\|")
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace('.', "\\.")
            .replace('!', "\\!")
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
