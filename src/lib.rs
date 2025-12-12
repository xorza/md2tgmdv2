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
    link_dest_url: String,
    buffer: String,
}

#[derive(Debug, Clone)]
enum Descriptor {
    Strong,
    Emphasis,
    #[allow(dead_code)]
    CodeBlock(String),
    Strikethrough,
    Code,
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
            link_dest_url: String::new(),
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
                    if self.link_dest_url.is_empty() {
                        self.output(&txt, true);
                    } else {
                        let txt = escape_text(&txt);
                        let url = escape_text(&self.link_dest_url);
                        self.output_unbreakable(&format!("[{}]({})", txt, url), false);

                        self.link_dest_url.clear();
                    }

                    println!("Text {}", txt);
                }
                Event::Code(txt) => {
                    self.stack.push(Descriptor::Code);
                    self.output("`", false);
                    self.output(&txt, true);
                    self.output_closing("`", false);
                    self.close_descriptor(Descriptor::Code)?;

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
                    self.new_line();
                    self.output("————————", true);
                    self.add_new_line = true;

                    println!("Rule");
                }
                Event::TaskListMarker(b) => {
                    self.new_line();
                    if b {
                        self.output("☑️", false);
                    } else {
                        self.output("☐", false);
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

    fn new_line(&mut self) {
        let last_len = self.result.last().map(|s| s.len()).unwrap_or(0);
        if last_len == 0 {
            return;
        }

        let needed = 1 + self.quote_level as usize;
        if last_len + needed > self.max_len {
            // Start a fresh chunk instead of emitting an empty newline-only tail.
            self.split_chunk();
            return;
        }

        let last = self.result.last_mut().unwrap();
        last.push('\n');
        if self.quote_level > 0 {
            last.push_str(&">".repeat(self.quote_level as usize));
        }
    }
    fn output(&mut self, txt: &str, escape: bool) {
        self.output_with_skip(txt, escape, false);
    }

    /// Emit text that should stay together in a single chunk (e.g., full links).
    /// If the text itself exceeds the chunk size, fall back to splitting at the
    /// available boundary to maintain the Telegram limit.
    fn output_unbreakable(&mut self, txt: &str, escape: bool) {
        let owned = if escape {
            escape_text(txt)
        } else {
            txt.to_string()
        };

        let mut remaining = owned.as_str();
        while !remaining.is_empty() {
            let pending_prefix = self.pending_prefix_len();
            let closers_len = self.closers_len(false);
            let current_len = self.result.last().map(|s| s.len()).unwrap_or(0);

            if current_len + pending_prefix + closers_len >= self.max_len {
                self.split_chunk();
                continue;
            }

            let available = self.max_len - current_len - pending_prefix - closers_len;
            let take = if remaining.len() <= available {
                remaining.len()
            } else if remaining.len() > self.max_len {
                // Unbreakable text longer than a whole chunk: split at the limit.
                available
            } else {
                0
            };

            if take == 0 {
                self.split_chunk();
                continue;
            }

            self.flush_pending_prefix();
            let last = self.result.last_mut().unwrap();
            last.push_str(&remaining[..take]);
            remaining = &remaining[take..];

            if !remaining.is_empty() {
                self.split_chunk();
            }
        }
    }

    /// Write a closing marker for the currently open top descriptor.
    /// This skips reserving space for that descriptor's own closer,
    /// so we don't over-reserve and force an unnecessary split.
    fn output_closing(&mut self, txt: &str, escape: bool) {
        self.output_with_skip(txt, escape, true);
    }

    fn output_with_skip(&mut self, txt: &str, escape: bool, skip_top: bool) {
        let owned = if escape {
            escape_text(txt)
        } else {
            txt.to_string()
        };

        let mut remaining = owned.as_str();
        while !remaining.is_empty() {
            // Reserve room for pending prefix and required closers so we never
            // overflow the chunk once we have to emit closing markers.
            let pending_prefix = self.pending_prefix_len();
            let closers_len = self.closers_len(skip_top);
            let current_len = self.result.last().map(|s| s.len()).unwrap_or(0);

            if current_len + pending_prefix + closers_len >= self.max_len {
                self.split_chunk();
                continue;
            }

            let available = self.max_len - current_len - pending_prefix - closers_len;
            if available == 0 {
                self.split_chunk();
                continue;
            }

            let take = split_point(remaining, available);
            if take == 0 {
                // No safe split point within available space: start a new chunk.
                self.split_chunk();
                continue;
            }
            let (part, rest) = remaining.split_at(take);

            self.flush_pending_prefix();

            // When we split, drop whitespace that straddles the boundary to avoid
            // trailing spaces in the previous chunk or leading spaces in the next.
            // Keep newlines intact so code blocks and wrapped text remain correct.
            let soft_ws = |c: char| c == ' ' || c == '\t';
            let (part, rest) = if rest.is_empty() {
                (part, rest)
            } else {
                (
                    part.trim_end_matches(soft_ws),
                    rest.trim_start_matches(soft_ws),
                )
            };

            if !part.is_empty() {
                let last = self.result.last_mut().unwrap();
                last.push_str(part);
            }

            remaining = rest;

            if !remaining.is_empty() {
                // We still have content left; close and reopen formatting for the next chunk.
                self.split_chunk();
            }
        }
    }

    /// Number of prefix characters that would be inserted before the next write.
    fn pending_prefix_len(&self) -> usize {
        let mut len = 0;
        if self.add_new_line {
            len += 1; // the newline itself
            if self.quote_level > 0 {
                len += self.quote_level as usize;
            }
        } else if self.result.last().map(|s| s.is_empty()).unwrap_or(true) && self.quote_level > 0 {
            len += self.quote_level as usize;
        }

        len
    }

    /// Emit any pending newline and quote prefix.
    fn flush_pending_prefix(&mut self) {
        let last = self.result.last_mut().unwrap();
        if self.add_new_line {
            last.push('\n');
            if self.quote_level > 0 {
                last.push_str(&">".repeat(self.quote_level as usize));
            }
            self.add_new_line = false;
        } else if last.is_empty() && self.quote_level > 0 {
            last.push_str(&">".repeat(self.quote_level as usize));
        }
    }

    fn split_chunk(&mut self) {
        if let Some(last) = self.result.last_mut() {
            while last.ends_with(' ') || last.ends_with('\t') {
                last.pop();
            }
        }
        self.write_closers();
        self.result.push(String::new());
        self.add_new_line = false;
        self.reopen_descriptors();
    }

    fn write_closers(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        let closers: Vec<&str> = self.stack.iter().rev().map(descriptor_closer).collect();
        let last = self.result.last_mut().unwrap();
        for closer in closers {
            last.push_str(closer);
        }
    }

    fn reopen_descriptors(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        // Clone to avoid holding an immutable borrow while writing.
        let descriptors = self.stack.clone();
        for desc in descriptors {
            match desc {
                Descriptor::Strong => self.output("**", false),
                Descriptor::Emphasis => self.output("_", false),
                Descriptor::Strikethrough => self.output("~~", false),
                Descriptor::Code => self.output("`", false),
                Descriptor::CodeBlock(lang) => {
                    self.output("```", false);
                    self.output(&lang, true);
                    self.add_new_line = true;
                }
            }
        }
    }

    fn closers_len(&self, skip_top: bool) -> usize {
        let mut iter = self.stack.iter().rev();
        if skip_top {
            iter.next();
        }
        iter.map(descriptor_closer).map(str::len).sum()
    }

    fn start_tag(&mut self, tag: Tag) -> anyhow::Result<()> {
        match tag {
            Tag::Paragraph => {
                self.new_line();

                println!("Paragraph");
            }
            Tag::Heading { level, .. } => {
                self.new_line();
                match level {
                    HeadingLevel::H1 => self.output("**🌟 ", false),
                    HeadingLevel::H2 => self.output("**⭐ ", false),
                    HeadingLevel::H3 => self.output("**✨ ", false),
                    HeadingLevel::H4 => self.output("**🔸 ", false),
                    HeadingLevel::H5 => self.output("_🔹 ", false),
                    HeadingLevel::H6 => self.output("_✴️ ", false),
                }

                println!("Heading");
            }
            Tag::BlockQuote(_) => {
                self.quote_level += 1;

                println!("BlockQuote");
            }
            Tag::CodeBlock(kind) => {
                if let Some(last) = self.result.last() {
                    if !last.is_empty() {
                        self.split_chunk();
                    }
                }
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.output("```", false);
                self.output(&lang, true);
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
                self.new_line();
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
                self.output("**", false);
                self.stack.push(Descriptor::Strong);

                println!("Strong");
            }
            Tag::Strikethrough => {
                self.output("~~", false);
                self.stack.push(Descriptor::Strikethrough);

                println!("Strikethrough");
            }
            Tag::Link { dest_url, .. } => {
                assert!(self.link_dest_url.is_empty());

                self.link_dest_url = dest_url.to_string();

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
                    HeadingLevel::H1 => self.output_closing("**", false),
                    HeadingLevel::H2 => self.output_closing("**", false),
                    HeadingLevel::H3 => self.output_closing("**", false),
                    HeadingLevel::H4 => self.output_closing("**", false),
                    HeadingLevel::H5 => self.output_closing("_", false),
                    HeadingLevel::H6 => self.output_closing("_", false),
                }
                self.add_new_line = true;

                println!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                self.add_new_line = true;
                self.quote_level -= 1;

                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                self.output_closing("```", false);
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
                self.output_closing("_", false);
                self.close_descriptor(Descriptor::Emphasis)?;

                println!("EndEmphasis");
            }
            TagEnd::Strong => {
                self.output_closing("**", false);
                self.close_descriptor(Descriptor::Strong)?;

                println!("EndStrong");
            }
            TagEnd::Strikethrough => {
                self.output_closing("~~", false);
                self.close_descriptor(Descriptor::Strikethrough)?;
            }
            TagEnd::Link => {
                println!("EndLink");
            }
            TagEnd::Image => {
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

    fn close_descriptor(&mut self, descriptor: Descriptor) -> anyhow::Result<()> {
        let last = self.stack.pop().expect("Unexpected end tag");
        assert_eq!(last, descriptor, "Unexpected end tag");

        Ok(())
    }
}

fn split_point(text: &str, max_len: usize) -> usize {
    if text.len() <= max_len {
        return text.len();
    }

    let mut last_space = None;
    for (idx, ch) in text.char_indices() {
        if idx >= max_len {
            break;
        }
        if ch.is_whitespace() {
            last_space = Some(idx + ch.len_utf8());
        }
    }

    last_space.unwrap_or(0)
}

fn descriptor_closer(desc: &Descriptor) -> &'static str {
    match desc {
        Descriptor::Strong => "**",
        Descriptor::Emphasis => "_",
        Descriptor::Strikethrough => "~~",
        Descriptor::Code => "`",
        Descriptor::CodeBlock(_) => "```",
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
            (Descriptor::Code, Descriptor::Code) => true,
            (Descriptor::Strikethrough, Descriptor::Strikethrough) => true,
            _ => unimplemented!(),
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
