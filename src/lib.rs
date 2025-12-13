//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

#![allow(unused_imports)]

use anyhow::anyhow;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::{hash::BuildHasher, ops::Range};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;

#[derive(Debug)]
pub struct Converter {
    max_len: usize,
    result: Vec<String>,
    stack: Vec<Descriptor>,
    quote_level: u8,
    link: Option<Link>,
    new_line: bool,

    // use for operations on temporary strings to avoid allocations
    buffer: String,
}

#[derive(Debug)]
pub struct Link {
    url: String,
    title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Descriptor {
    Strong,
    Emphasis,
    CodeBlock(String),
    Strikethrough,
    Code,
    List { ordered: bool, index: u32 },
    Heading(u8),
    ListItem,
}

impl Default for Converter {
    fn default() -> Self {
        Self {
            max_len: TELEGRAM_BOT_MAX_MESSAGE_LENGTH,
            result: vec![],

            stack: Vec::new(),
            quote_level: 0,
            link: None,
            new_line: true,

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

    fn new_line(&mut self) {
        let last = self.result.last_mut().unwrap();
        last.push_str("\n");
        if self.quote_level > 0 {
            last.push_str(&">".repeat(self.quote_level as usize));
        }
        self.new_line = true;
    }

    fn url(&mut self, txt: &str, url: &str) {
        let txt = escape_text(txt);
        let url = escape_text(url);
        self.text(&format!("[{}]({})", txt, url));
    }

    fn text(&mut self, txt: &str) {
        let prefix = self.build_prefix();
        let last = self.result.last_mut().unwrap();
        last.push_str(&prefix);
        last.push_str(txt);
        self.new_line = false;
    }

    fn postfix(&mut self, desc: Descriptor) {
        let last = self.result.last_mut().unwrap();
        match desc {
            Descriptor::ListItem => {}
            Descriptor::List { .. } => {}
            Descriptor::CodeBlock { .. } => {
                last.push_str("```\n");
            }
            Descriptor::Code => {
                last.push_str("`");
            }
            Descriptor::Heading(level) => {
                last.push_str("\n");
            }
            Descriptor::Emphasis => {
                last.push_str("*");
            }
            Descriptor::Strong => {
                last.push_str("*");
            }
            Descriptor::Strikethrough => {
                last.push_str("~");
            }
        }
    }

    fn build_prefix(&mut self) -> String {
        if !self.new_line {
            return String::new();
        }

        // Clear reusable buffer that will hold the suffix.
        self.buffer.clear();

        // Render list indentation + bullet when we are inside a list item
        if self.stack.iter().any(|d| matches!(d, Descriptor::ListItem)) {
            // Depth = number of lists currently on the stack.
            let depth = self
                .stack
                .iter()
                .filter(|d| matches!(d, Descriptor::List { .. }))
                .count();

            // Two spaces per nesting level (skip the first level).
            if depth > 1 {
                for _ in 0..((depth - 1) * 2) {
                    self.buffer.push(' ');
                }
            }
            let index = self.stack.iter_mut().rev().find_map(|d| match d {
                Descriptor::List {
                    ordered: true,
                    index,
                } => Some(index),
                _ => None,
            });
            if let Some(index) = index {
                self.buffer.push_str(&format!("{}\\. ", index));
                *index += 1;
            } else {
                self.buffer.push_str("⦁ ");
            }
        }

        // Build prefix and suffix for inline formatting. We open from outermost to
        // innermost, and close in reverse order by accumulating into `self.buffer`.
        for desc in self.stack.iter() {
            match desc {
                Descriptor::Heading(level) => {
                    let marker = match level {
                        1 => "*🌟 ",
                        2 => "*⭐ ",
                        3 => "*✨ ",
                        4 => "*🔸 ",
                        5 => "_🔹 ",
                        _ => "_✴️ ",
                    };
                    self.buffer.push_str(marker);
                }
                Descriptor::CodeBlock(lang) => {
                    let lang = escape_text(lang);
                    self.buffer.push_str("```");
                    self.buffer.push_str(&lang);
                    self.buffer.push('\n');
                }
                Descriptor::Strong => {
                    self.buffer.push_str("*");
                }
                Descriptor::Emphasis => {
                    self.buffer.push('_');
                }
                Descriptor::Strikethrough => {
                    self.buffer.push('~');
                }
                Descriptor::Code => {
                    self.buffer.push('`');
                }
                Descriptor::List { .. } | Descriptor::ListItem => {
                    // Already handled indentation/bullet above.
                }
            }
        }

        return std::mem::take(&mut self.buffer);
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
                    let txt = escape_text(&txt);
                    match self.link.as_mut() {
                        Some(link) => {
                            link.title.push_str(&txt);
                        }
                        None => self.text(&txt),
                    }

                    println!("Text {}", txt);
                }
                Event::Code(txt) => {
                    self.stack.push(Descriptor::Code);
                    self.text(&escape_text(&txt));
                    self.stack.pop();

                    println!("Code");
                }
                Event::InlineMath(txt) => {
                    self.text(&escape_text(&txt));

                    println!("InlineMath");
                }
                Event::DisplayMath(txt) => {
                    self.text(&escape_text(&txt));

                    println!("DisplayMath");
                }
                Event::Html(txt) => {
                    self.text(&escape_text(&txt));

                    println!("Html");
                }
                Event::InlineHtml(txt) => {
                    self.text(&escape_text(&txt));

                    println!("InlineHtml");
                }
                Event::FootnoteReference(txt) => {
                    self.text(&escape_text(&txt));

                    println!("FootnoteReference");
                }
                Event::SoftBreak => {
                    self.new_line();

                    println!("SoftBreak");
                }
                Event::HardBreak => {
                    self.new_line();

                    println!("HardBreak");
                }
                Event::Rule => {
                    self.new_line();
                    self.text("———");
                    self.new_line();
                    self.new_line();

                    println!("Rule");
                }
                Event::TaskListMarker(b) => {
                    if b {
                        self.text("☑️");
                    } else {
                        self.text("☐");
                    }

                    println!("TaskListMarker({})", b);
                }
            }
        }

        if !self.stack.is_empty() {
            return Err(anyhow!("Unbalanced tags"));
        }

        for chunk in &mut self.result {
            let trimmed_len = chunk.trim_end().len();
            chunk.truncate(trimmed_len);
        }

        Ok(std::mem::take(&mut self.result))
    }

    fn start_tag(&mut self, tag: Tag) -> anyhow::Result<()> {
        match tag {
            Tag::Paragraph => {
                println!("Paragraph");
            }
            Tag::Heading { level, .. } => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.stack.push(Descriptor::Heading(level));

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
                self.stack.push(Descriptor::CodeBlock(lang));

                println!("CodeBlock");
            }
            Tag::HtmlBlock => {
                println!("HtmlBlock");
            }
            Tag::List(list) => {
                let desc = Descriptor::List {
                    ordered: list.is_some(),
                    index: list.unwrap_or(1) as u32,
                };
                self.stack.push(desc);

                println!("List");
            }
            Tag::Item => {
                self.stack.push(Descriptor::ListItem);

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
                self.stack.push(Descriptor::Emphasis);

                println!("Emphasis");
            }
            Tag::Strong => {
                self.stack.push(Descriptor::Strong);

                println!("Strong");
            }
            Tag::Strikethrough => {
                self.stack.push(Descriptor::Strikethrough);

                println!("Strikethrough");
            }
            Tag::Link { dest_url, .. } => {
                assert!(self.link.is_none());
                self.link = Some(Link {
                    url: dest_url.to_string(),
                    title: String::new(),
                });

                println!("Link");
            }
            Tag::Image { dest_url, .. } => {
                assert!(self.link.is_none());
                self.link = Some(Link {
                    url: dest_url.to_string(),
                    title: String::new(),
                });

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
                self.new_line();

                println!("EndParagraph");
            }
            TagEnd::Heading(_) => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::Heading { .. }),
                    "Unexpected end of list"
                );
                self.postfix(desc);

                println!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                self.quote_level -= 1;

                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::CodeBlock { .. }),
                    "Unexpected end of list"
                );
                self.postfix(desc);

                println!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                println!("EndHtmlBlock");
            }
            TagEnd::List(_) => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::List { .. }),
                    "Unexpected end of list"
                );
                self.postfix(desc);

                println!("EndList");
            }
            TagEnd::Item => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::ListItem),
                    "Unexpected end of list"
                );
                self.new_line();

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
                self.new_line();

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
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::Emphasis),
                    "Unexpected end of list"
                );
                self.postfix(desc);

                println!("EndEmphasis");
            }
            TagEnd::Strong => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(matches!(desc, Descriptor::Strong), "Unexpected end of list");
                self.postfix(desc);

                println!("EndStrong");
            }
            TagEnd::Strikethrough => {
                let desc = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(desc, Descriptor::Strikethrough),
                    "Unexpected end of list"
                );
                self.postfix(desc);
            }
            TagEnd::Link => {
                let link = self.link.take().expect("Unexpected end of list");
                self.url(&link.title, &link.url);

                println!("EndLink");
            }
            TagEnd::Image => {
                let link = self.link.take().expect("Unexpected end of list");
                let title = if link.title.is_empty() {
                    "Image".to_string()
                } else {
                    link.title.clone()
                };
                self.url(&title, &link.url);

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
