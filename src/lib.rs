//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

#![allow(unused_imports)]

use anyhow::anyhow;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

#[derive(Debug)]
pub struct Converter {
    max_len: usize,
    result: Vec<String>,
    stack: Vec<Descriptor>,
    add_new_line: bool,
    quote_level: u8,
    list: bool,
}

#[derive(Debug, Clone)]
enum Descriptor {
    Strong,
    Emphasis,
    CodeBlock(String),
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            max_len: TELEGRAM_BOT_MAX_MESSAGE_LENGTH,
            result: vec![],
            stack: Vec::new(),
            add_new_line: false,
            quote_level: 0,
            list: false,
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

        self.result.push(String::new());

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
                    self.output(&txt, true);

                    println!("Text {}", txt);
                }
                Event::Code(txt) => {
                    self.output("`", false);
                    self.output(&txt, true);
                    self.output("`", false);

                    println!("Code");
                }
                Event::InlineMath(txt) => {
                    self.output(&txt, true);

                    println!("InlineMath");
                }
                Event::DisplayMath(txt) => {
                    self.output(&txt, true);

                    println!("DisplayMath");
                }
                Event::Html(txt) => {
                    self.output(&txt, true);

                    println!("Html");
                }
                Event::InlineHtml(txt) => {
                    self.output(&txt, true);

                    println!("InlineHtml");
                }
                Event::FootnoteReference(txt) => {
                    self.output(&txt, true);

                    println!("FootnoteReference");
                }
                Event::SoftBreak => {
                    self.add_new_line = true;

                    println!("SoftBreak");
                }
                Event::HardBreak => {
                    self.add_new_line = true;

                    println!("HardBreak");
                }
                Event::Rule => {
                    self.output_new_line();
                    self.output("————————", true);
                    self.add_new_line = true;

                    println!("Rule");
                }
                Event::TaskListMarker(b) => {
                    self.output_new_line();
                    if b {
                        self.output("[x]", false);
                    } else {
                        self.output("[ ]", false);
                    }

                    println!("TaskListMarker({})", b);
                }
            }
        }

        if !self.stack.is_empty() {
            return Err(anyhow!("Unbalanced tags"));
        }

        Ok(std::mem::take(&mut self.result))
    }

    fn output_new_line(&mut self) {
        self.output("\n", false);
    }
    fn output(&mut self, txt: &str, escape: bool) {
        let last = self.result.last_mut().unwrap();

        if self.add_new_line {
            last.push_str("\n");
            if self.quote_level > 0 {
                last.push_str(&">".repeat(self.quote_level as usize));
            }
            self.add_new_line = false;
        }

        if txt == "\n" {
            if !last.is_empty() {
                last.push_str("\n");
            }
            if self.quote_level > 0 {
                last.push_str(&">".repeat(self.quote_level as usize));
            }
            return;
        }

        if last.is_empty() && self.quote_level > 0 {
            last.push_str(&">".repeat(self.quote_level as usize));
        }

        if escape {
            let escaped = escape_text(&txt);
            last.push_str(&escaped);
        } else {
            last.push_str(txt);
        }
    }

    fn start_tag(&mut self, tag: Tag) -> anyhow::Result<()> {
        match tag {
            Tag::Paragraph => {
                self.output_new_line();

                println!("Paragraph");
            }
            Tag::Heading { level, .. } => {
                match level {
                    HeadingLevel::H1 => self.output("*⭐⭐ ", false),
                    HeadingLevel::H2 => self.output("*⭐ ", false),
                    HeadingLevel::H3 => self.output("*", false),
                    HeadingLevel::H4 => self.output("*", false),
                    HeadingLevel::H5 => self.output("", false),
                    HeadingLevel::H6 => self.output("", false),
                }

                println!("Heading");
            }
            Tag::BlockQuote(_) => {
                self.quote_level += 1;

                println!("BlockQuote");
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.output("```", false);
                self.output(&lang, false);
                self.add_new_line = true;
                self.stack.push(Descriptor::CodeBlock(lang));

                println!("CodeBlock");
            }
            Tag::HtmlBlock => {
                println!("HtmlBlock");
            }
            Tag::List(_) => {
                self.list = true;

                println!("List");
            }
            Tag::Item => {
                self.output_new_line();
                self.output("⦁ ", false);

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
                self.output("_", false);
                self.stack.push(Descriptor::Emphasis);

                println!("Emphasis");
            }
            Tag::Strong => {
                self.output("*", false);
                self.stack.push(Descriptor::Strong);

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

        Ok(())
    }

    fn end_tag(&mut self, tag: TagEnd) -> anyhow::Result<()> {
        match tag {
            TagEnd::Paragraph => {
                self.add_new_line = true;

                println!("EndParagraph");
            }
            TagEnd::Heading(level) => {
                match level {
                    HeadingLevel::H1 => self.output("*", false),
                    HeadingLevel::H2 => self.output("*", false),
                    HeadingLevel::H3 => self.output("*", false),
                    HeadingLevel::H4 => self.output("*", false),
                    HeadingLevel::H5 => self.output("", false),
                    HeadingLevel::H6 => self.output("", false),
                }
                self.add_new_line = true;

                println!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                self.add_new_line = true;

                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                self.output("```", false);
                self.add_new_line = true;
                self.close_descriptor(Descriptor::CodeBlock(String::new()))?;

                println!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                println!("EndHtmlBlock");
            }
            TagEnd::List(_) => {
                self.list = false;
                self.add_new_line = true;

                println!("EndList");
            }
            TagEnd::Item => {
                // self.add_new_line = true;

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
                self.output("_", false);
                self.close_descriptor(Descriptor::Emphasis)?;

                println!("EndEmphasis");
            }
            TagEnd::Strong => {
                self.output("*", false);
                self.close_descriptor(Descriptor::Strong)?;

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
        let last = self.stack.pop().expect("Unexpected end tag");
        assert_eq!(last, descriptor, "Unexpected end tag");

        Ok(())
    }
}
fn escape_text(text: &str) -> String {
    // Single pass escape to avoid O(n*k) replace chaining.
    let mut out = String::with_capacity(text.len() * 2); // worst case every char escapes
    for ch in text.chars() {
        match ch {
            '\\' | '*' | '_' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+' | '-' | '='
            | '|' | '{' | '}' | '.' | '!' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

impl PartialEq for Descriptor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Descriptor::Strong, Descriptor::Strong) => true,
            (Descriptor::Emphasis, Descriptor::Emphasis) => true,
            (Descriptor::CodeBlock(_), Descriptor::CodeBlock(_)) => true,
            _ => false,
        }
    }
}

impl Eq for Descriptor {}

// #[test]
// fn test() -> anyhow::Result<()> {
//     let text = include_str!("../tests/1-input.md");
//     let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

//     let text = include_str!("../tests/3-input.md");
//     let _ = transform(text, TELEGRAM_BOT_MAX_MESSAGE_LENGTH)?;

//     Ok(())
// }
