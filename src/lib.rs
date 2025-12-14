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
    stack: Vec<Frame>,
    quote_level: u8,
    link: Option<Link>,
    new_line: bool,
    prefix: String,
    // use for operations on temporary strings to avoid allocations
    // buffer: String,
}

#[derive(Debug)]
pub struct Link {
    url: String,
    title: String,
}

#[derive(Debug, Clone)]
struct Frame {
    desc: Descriptor,
    open_marker: String,
    applied_in_chunk: bool,
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
            new_line: false,
            prefix: String::new(),
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
        self.new_line = true;
    }

    fn url(&mut self, txt: &str, url: &str) {
        let txt = escape_text(txt);
        let url = escape_text(url);
        self.text(&format!("[{}]({})", txt, url));
    }

    fn closing_marker(&self, desc: &Descriptor) -> String {
        match desc {
            Descriptor::ListItem => String::new(),
            Descriptor::List { .. } => String::new(),
            Descriptor::CodeBlock(_) => "```\n".to_string(),
            Descriptor::Code => "`".to_string(),
            Descriptor::Heading(level) => {
                let marker = match level {
                    1 | 2 | 3 | 4 => "*",
                    5 | _ => "_",
                };
                marker.to_string()
            }
            Descriptor::Emphasis => "_".to_string(),
            Descriptor::Strong => "*".to_string(),
            Descriptor::Strikethrough => "~".to_string(),
        }
    }

    fn build_open_marker(&mut self, desc: &Descriptor) -> String {
        match desc {
            Descriptor::ListItem => {
                let depth = self
                    .stack
                    .iter()
                    .filter(|f| matches!(f.desc, Descriptor::List { .. }))
                    .count();

                let mut marker = String::new();
                // Two spaces per nesting level (skip the first level).
                if depth > 1 {
                    for _ in 0..((depth - 1) * 2) {
                        marker.push(' ');
                    }
                }

                let (ordered, index) = self
                    .stack
                    .iter_mut()
                    .rev()
                    .find_map(|f| match &mut f.desc {
                        Descriptor::List { ordered, index } => Some((*ordered, index)),
                        _ => None,
                    })
                    .expect("No list found");
                if ordered {
                    marker.push_str(&format!("{}\\. ", index));
                    *index += 1;
                } else {
                    marker.push_str("⦁ ");
                }

                marker
            }
            Descriptor::List { .. } => String::new(),
            Descriptor::CodeBlock(lang) => {
                let mut marker = String::from("```");
                if !lang.is_empty() {
                    marker.push_str(lang);
                }
                marker.push('\n');
                marker
            }
            Descriptor::Code => "`".to_string(),
            Descriptor::Heading(level) => {
                let marker = match level {
                    1 => "*🌟 ",
                    2 => "*⭐ ",
                    3 => "*✨ ",
                    4 => "*🔸 ",
                    5 => "_🔹 ",
                    _ => "_✴️ ",
                };
                marker.to_string()
            }
            Descriptor::Emphasis => "_".to_string(),
            Descriptor::Strong => "*".to_string(),
            Descriptor::Strikethrough => "~".to_string(),
        }
    }

    fn push_frame(&mut self, desc: Descriptor) {
        let open_marker = self.build_open_marker(&desc);
        self.prefix.push_str(&open_marker);
        self.stack.push(Frame {
            desc,
            open_marker,
            applied_in_chunk: false,
        });
    }

    fn close_frame(&mut self, frame: Frame) {
        if frame.applied_in_chunk {
            let marker = self.closing_marker(&frame.desc);
            if let Some(last) = self.result.last_mut() {
                last.push_str(&marker);
            }
        } else if !frame.open_marker.is_empty() && self.prefix.ends_with(&frame.open_marker) {
            let new_len = self.prefix.len() - frame.open_marker.len();
            self.prefix.truncate(new_len);
        }
    }

    fn all_postfix_len(&self) -> usize {
        self.stack
            .iter()
            .map(|f| self.closing_marker(&f.desc).len())
            .sum()
    }

    fn append_postfix_for_applied(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        let mut suffix = String::new();
        for frame in self.stack.iter().rev() {
            if frame.applied_in_chunk {
                suffix.push_str(&self.closing_marker(&frame.desc));
            }
        }
        if !suffix.is_empty() {
            if let Some(last) = self.result.last_mut() {
                last.push_str(&suffix);
            }
        }
    }

    fn rebuild_prefix_from_stack(&mut self) {
        self.prefix.clear();
        for frame in &self.stack {
            self.prefix.push_str(&frame.open_marker);
        }
    }

    fn mark_pending_prefix_applied(&mut self) {
        if self.prefix.is_empty() {
            return;
        }
        for frame in self.stack.iter_mut() {
            if !frame.applied_in_chunk {
                frame.applied_in_chunk = true;
            }
        }
        self.prefix.clear();
    }

    fn close_current_chunk(&mut self) {
        // add postfixes for everything that is currently opened in this chunk
        self.append_postfix_for_applied();

        // start new chunk
        self.result.push(String::new());

        // reopen prefixes in the next chunk
        for frame in self.stack.iter_mut() {
            frame.applied_in_chunk = false;
        }
        self.rebuild_prefix_from_stack();
    }

    fn first_word_len(text: &str) -> usize {
        let mut started = false;
        let mut len = 0;
        for ch in text.chars() {
            if ch.is_whitespace() {
                if started {
                    break;
                } else {
                    continue;
                }
            }
            started = true;
            len += ch.len_utf8();
        }
        len
    }

    /// If the start of `text` is a Markdown link `[...] (...)` return its byte length.
    /// Escaped parentheses inside are ignored when searching for the closing `)`.
    fn unsplittable_link_len(text: &str) -> Option<usize> {
        let bytes = text.as_bytes();
        if bytes.first().copied() != Some(b'[') {
            return None;
        }

        let mut i = 1;
        let len = bytes.len();
        let mut mid = None;
        while i + 1 < len {
            match bytes[i] {
                b'\\' => {
                    i += 2;
                    continue;
                }
                b']' if bytes[i + 1] == b'(' => {
                    mid = Some(i);
                    i += 2;
                    break;
                }
                _ => i += 1,
            }
        }

        if mid.is_none() {
            return None;
        }

        while i < len {
            match bytes[i] {
                b'\\' => {
                    i += 2;
                    continue;
                }
                b')' => return Some(i + 1),
                _ => i += 1,
            }
        }

        None
    }

    /// If text starts with an url-like token (http/https) return its length until whitespace.
    fn unsplittable_url_like_len(text: &str) -> Option<usize> {
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(Converter::first_word_len(text));
        }
        None
    }

    fn leading_whitespace_len(text: &str) -> usize {
        let mut len = 0;
        for ch in text.chars() {
            if ch.is_whitespace() {
                len += ch.len_utf8();
            } else {
                break;
            }
        }
        len
    }

    fn find_split_point(text: &str, limit: usize) -> (usize, Option<char>) {
        if text.len() <= limit {
            return (text.len(), None);
        }

        let mut last_ws: Option<(usize, char)> = None;
        for (idx, ch) in text.char_indices() {
            if idx > limit {
                break;
            }
            if ch.is_whitespace() {
                if idx > 0 {
                    last_ws = Some((idx, ch));
                }
            }
        }

        if let Some((idx, ch)) = last_ws {
            (idx, Some(ch))
        } else {
            (limit, None)
        }
    }

    fn text(&mut self, txt: &str) {
        if txt.is_empty() {
            let last_len = self.result.last().map(|s| s.len()).unwrap_or(0);
            let mut newline = String::new();
            if self.new_line {
                if last_len > 0 {
                    newline.push('\n');
                }
                if self.quote_level > 0 {
                    newline.push_str(&">".repeat(self.quote_level as usize));
                }
                self.new_line = false;
            }
            if !newline.is_empty() || !self.prefix.is_empty() {
                if let Some(last) = self.result.last_mut() {
                    last.push_str(&newline);
                    last.push_str(&self.prefix);
                }
                self.mark_pending_prefix_applied();
            }
            return;
        }

        let mut remaining = txt;

        while !remaining.is_empty() {
            let last_len = self.result.last().map(|s| s.len()).unwrap_or(0);
            let mut newline = String::new();
            if self.new_line {
                if last_len > 0 {
                    newline.push('\n');
                }
                if self.quote_level > 0 {
                    newline.push_str(&">".repeat(self.quote_level as usize));
                }
            }

            let prefix_str = self.prefix.clone();
            let closing_len_if_applied = self.all_postfix_len();

            let leading_ws = Converter::leading_whitespace_len(remaining);
            let after_ws = &remaining[leading_ws..];

            let token_len;
            let mut unsplittable = false;

            if let Some(len) = Converter::unsplittable_link_len(after_ws) {
                token_len = leading_ws + len;
                unsplittable = true;
            } else if let Some(len) = Converter::unsplittable_url_like_len(after_ws) {
                token_len = leading_ws + len;
                unsplittable = true;
            } else {
                let first_word = Converter::first_word_len(after_ws);
                if first_word == 0 {
                    // No visible text, just flush pending prefix/newline and return.
                    if !newline.is_empty() || !prefix_str.is_empty() {
                        let last = self.result.last_mut().unwrap();
                        last.push_str(&newline);
                        last.push_str(&prefix_str);
                        self.mark_pending_prefix_applied();
                        self.new_line = false;
                    }
                    break;
                }
                token_len = leading_ws + first_word;
            }

            if last_len + newline.len() + prefix_str.len() + closing_len_if_applied + token_len
                > self.max_len
            {
                // Nothing fits, split before the newline.
                self.close_current_chunk();
                continue;
            }

            let available = self.max_len
                - (last_len + newline.len() + prefix_str.len() + closing_len_if_applied);

            if unsplittable && token_len > available {
                // Move to next chunk to keep token intact.
                self.close_current_chunk();
                continue;
            }

            let (split_at, split_char) = if unsplittable {
                (token_len.min(available), None)
            } else {
                Converter::find_split_point(remaining, available)
            };
            let first_part = &remaining[..split_at];

            {
                let last = self.result.last_mut().unwrap();
                last.push_str(&newline);
                last.push_str(&prefix_str);
                last.push_str(first_part);
            }

            if self.new_line {
                self.new_line = false;
            }
            self.mark_pending_prefix_applied();

            let mut next_start = split_at;
            if let Some(ch) = split_char {
                next_start += ch.len_utf8();
            }

            if next_start < remaining.len() {
                // Not all text fitted, move to new chunk.
                self.close_current_chunk();
                remaining = &remaining[next_start..];
                continue;
            }

            break;
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
                    let desc = Descriptor::Code;
                    self.push_frame(desc.clone());
                    self.text(&escape_text(&txt));
                    let frame = self.stack.pop().expect("Unexpected end of list");
                    assert!(frame.desc == desc, "Unexpected end of list");
                    self.close_frame(frame);

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
                    self.text("");
                    self.new_line();
                    self.text("");
                    self.text("———");
                    self.new_line();
                    self.text("");
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
                self.new_line();

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
                let desc = Descriptor::Heading(level);
                self.push_frame(desc);

                println!("Heading");
            }
            Tag::BlockQuote(_) => {
                self.quote_level += 1;
                self.new_line();
                // self.prefix.push('>');

                println!("BlockQuote");
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let desc = Descriptor::CodeBlock(lang);
                self.push_frame(desc);

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
                self.push_frame(desc);

                println!("List {:?}", list);
            }
            Tag::Item => {
                self.push_frame(Descriptor::ListItem);
                self.new_line();

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
                self.push_frame(Descriptor::Emphasis);

                println!("Emphasis");
            }
            Tag::Strong => {
                self.push_frame(Descriptor::Strong);

                println!("Strong");
            }
            Tag::Strikethrough => {
                self.push_frame(Descriptor::Strikethrough);

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
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::Heading { .. }),
                    "Unexpected end of list"
                );
                self.close_frame(frame);
                self.new_line();

                println!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                self.quote_level -= 1;

                println!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::CodeBlock { .. }),
                    "Unexpected end of list"
                );
                self.close_frame(frame);

                println!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                println!("EndHtmlBlock");
            }
            TagEnd::List(_) => {
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::List { .. }),
                    "Unexpected end of list"
                );
                self.close_frame(frame);

                println!("EndList");
            }
            TagEnd::Item => {
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::ListItem),
                    "Unexpected end of list"
                );
                self.close_frame(frame);
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
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::Emphasis),
                    "Unexpected end of list"
                );
                self.close_frame(frame);

                println!("EndEmphasis");
            }
            TagEnd::Strong => {
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::Strong),
                    "Unexpected end of list"
                );
                self.close_frame(frame);

                println!("EndStrong");
            }
            TagEnd::Strikethrough => {
                let frame = self.stack.pop().expect("Unexpected end of list");
                assert!(
                    matches!(frame.desc, Descriptor::Strikethrough),
                    "Unexpected end of list"
                );
                self.close_frame(frame);
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
