//! md2tgmdv2 — Markdown → Telegram MarkdownV2 renderer.
//!
//! Public entry point is [`transform`]. It renders Markdown into Telegram‑safe
//! MarkdownV2 and splits the result into chunks that fit the provided limit.

use anyhow::anyhow;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Telegram MarkdownV2 message hard limit.
pub const TELEGRAM_BOT_MAX_MESSAGE_LENGTH: usize = 4096;
const DEBUG_LOG: bool = false;

macro_rules! debug_log {
    ($($arg:tt)*) => {
        if DEBUG_LOG {
            println!($($arg)*);
        }
    };
}

#[derive(Debug)]
pub struct Converter {
    max_len: usize,
    result: Vec<String>,
    stack: Vec<Descriptor>,
    add_new_line: bool,
    quote_level: u8,
    list: bool, // Deprecated; kept for backward compatibility, not used.
    list_stack: Vec<ListState>,
    carry_list_indent_levels: usize,
    after_list_prefix: bool,
    link_dest_url: String,
    buffer: String,
    // Depth counter for temporarily skipping events (used for image alt text).
    skip_depth: u16,
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
            list_stack: Vec::new(),
            carry_list_indent_levels: 0,
            after_list_prefix: false,
            link_dest_url: String::new(),
            buffer: String::new(),
            skip_depth: 0,
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
            if self.skip_depth > 0 {
                // When skipping (e.g., image alt text), keep depth balanced.
                match &event {
                    Event::Start(_) => self.skip_depth += 1,
                    Event::End(_) => self.skip_depth -= 1,
                    _ => {}
                }
                continue;
            }
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
                        self.buffer.clear();
                        self.buffer.push('[');
                        push_escaped(&mut self.buffer, &txt);
                        self.buffer.push_str("](");
                        push_escaped(&mut self.buffer, &self.link_dest_url);
                        self.buffer.push(')');

                        // Move the built link out to avoid holding an immutable borrow
                        // while mutably using `self` during output.
                        let mut link = String::new();
                        std::mem::swap(&mut self.buffer, &mut link);
                        self.output_unbreakable(link.as_str(), false);

                        // Return the owned buffer for reuse and leave it clean.
                        self.buffer = link;
                        self.buffer.clear();

                        self.link_dest_url.clear();
                    }

                    debug_log!("Text {}", txt);
                }
                Event::Code(txt) => {
                    self.stack.push(Descriptor::Code);
                    self.output("`", false);
                    self.output(&txt, true);
                    self.output_closing("`", false);
                    self.close_descriptor(Descriptor::Code)?;

                    debug_log!("Code");
                }
                Event::InlineMath(txt) => {
                    self.output(&txt, true);

                    debug_log!("InlineMath");
                }
                Event::DisplayMath(txt) => {
                    self.output(&txt, true);

                    debug_log!("DisplayMath");
                }
                Event::Html(txt) => {
                    self.output(&txt, true);

                    debug_log!("Html");
                }
                Event::InlineHtml(txt) => {
                    self.output(&txt, true);

                    debug_log!("InlineHtml");
                }
                Event::FootnoteReference(txt) => {
                    self.output(&txt, true);

                    debug_log!("FootnoteReference");
                }
                Event::SoftBreak => {
                    self.add_new_line = true;

                    debug_log!("SoftBreak");
                }
                Event::HardBreak => {
                    self.add_new_line = true;

                    debug_log!("HardBreak");
                }
                Event::Rule => {
                    self.new_line();
                    self.output("————————", true);
                    self.add_new_line = true;

                    debug_log!("Rule");
                }
                Event::TaskListMarker(b) => {
                    self.new_line();
                    if b {
                        self.output("☑️", false);
                    } else {
                        self.output("☐", false);
                    }

                    debug_log!("TaskListMarker({})", b);
                }
            }
        }

        if !self.stack.is_empty() {
            return Err(anyhow!("Unbalanced tags"));
        }

        for (idx, chunk) in self.result.iter().enumerate() {
            if chunk.len() > self.max_len {
                return Err(anyhow!(
                    "internal parser error: chunk {} exceeds max_len ({} > {})",
                    idx,
                    chunk.len(),
                    self.max_len
                ));
            }
        }

        Ok(std::mem::take(&mut self.result))
    }

    fn new_line(&mut self) {
        // Any newline ends the "after prefix" state to avoid suppressing later breaks.
        self.after_list_prefix = false;

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

    /// Escape text and move it out, leaving `buffer` empty for reuse.
    fn take_escaped(&mut self, txt: &str) -> String {
        self.buffer.clear();
        push_escaped(&mut self.buffer, txt);
        std::mem::take(&mut self.buffer)
    }

    /// Emit text that should stay together in a single chunk (e.g., full links).
    /// If the text itself exceeds the chunk size, fall back to splitting at the
    /// available boundary to maintain the Telegram limit.
    fn output_unbreakable(&mut self, txt: &str, escape: bool) {
        // Reuse the internal buffer for escaped text to limit allocations.
        let mut temp = None;
        let mut remaining: &str = if escape {
            temp = Some(self.take_escaped(txt));
            temp.as_ref().unwrap().as_str()
        } else {
            txt
        };
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
        if let Some(owned) = temp {
            self.buffer = owned;
            self.buffer.clear(); // leave buffer clean for the next use
        }
    }

    /// Write a closing marker for the currently open top descriptor.
    /// This skips reserving space for that descriptor's own closer,
    /// so we don't over-reserve and force an unnecessary split.
    fn output_closing(&mut self, txt: &str, escape: bool) {
        self.output_with_skip(txt, escape, true);
    }

    fn output_with_skip(&mut self, txt: &str, escape: bool, skip_top: bool) {
        // Reuse the internal buffer for escaped text to limit allocations.
        let mut temp = None;
        let mut remaining: &str = if escape {
            temp = Some(self.take_escaped(txt));
            temp.as_ref().unwrap().as_str()
        } else {
            txt
        };
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

            let mut take = split_point(remaining, available);
            let forced_split = if take == 0 {
                // No safe split point within available space: fall back to a hard split
                // at the available boundary to guarantee forward progress (prevents loops).
                take = available;
                true
            } else {
                false
            };
            let (part, rest) = remaining.split_at(take);

            self.flush_pending_prefix();

            // When we split, drop whitespace that straddles the boundary to avoid
            // trailing spaces in the previous chunk or leading spaces in the next.
            // Keep newlines intact so code blocks and wrapped text remain correct.
            let (part, rest) = if rest.is_empty() || forced_split {
                (part, rest)
            } else {
                let soft_ws = |c: char| c == ' ' || c == '\t';
                (
                    part.trim_end_matches(soft_ws),
                    rest.trim_start_matches(soft_ws),
                )
            };

            if part.is_empty() {
                // Dropped only whitespace; keep filling the current chunk.
                remaining = rest;
                continue;
            }

            let last = self.result.last_mut().unwrap();
            last.push_str(part);

            remaining = rest;

            if !remaining.is_empty() {
                // We still have content left; close and reopen formatting for the next chunk.
                self.split_chunk();
            }
        }
        if let Some(owned) = temp {
            self.buffer = owned;
            self.buffer.clear(); // leave buffer clean for the next use
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
                Descriptor::Strong => self.output("*", false),
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

    fn list_prefix(&mut self) -> (String, usize) {
        let base_levels = self.list_stack.len().saturating_sub(1);
        let extra_levels = self.list_stack.last().map(|s| s.extra_levels).unwrap_or(0);
        let indent_len = (base_levels + extra_levels) * 2;
        let indent = " ".repeat(indent_len);
        if let Some(state) = self.list_stack.last_mut() {
            if state.ordered {
                let number = state.start + state.items.len() as u64;
                return (format!("{}{}. ", indent, number), indent_len);
            }
        }
        (format!("{}⦁ ", indent), indent_len)
    }

    fn start_tag(&mut self, tag: Tag) -> anyhow::Result<()> {
        // Reset carry indent when encountering non-list content.
        match tag {
            Tag::List(_) => {}
            _ => self.carry_list_indent_levels = 0,
        }
        match tag {
            Tag::Paragraph => {
                // When a paragraph begins right after a list prefix, keep it
                // on the same line to avoid stray blank lines between the marker
                // and the first line of text.
                if self.after_list_prefix {
                    self.after_list_prefix = false;
                } else {
                    self.new_line();
                }

                debug_log!("Paragraph");
            }
            Tag::Heading { level, .. } => {
                self.new_line();
                match level {
                    HeadingLevel::H1 => self.output("*🌟 ", false),
                    HeadingLevel::H2 => self.output("*⭐ ", false),
                    HeadingLevel::H3 => self.output("*✨ ", false),
                    HeadingLevel::H4 => self.output("*🔸 ", false),
                    HeadingLevel::H5 => self.output("_🔹 ", false),
                    HeadingLevel::H6 => self.output("_✴️ ", false),
                }

                debug_log!("Heading");
            }
            Tag::BlockQuote(_) => {
                self.quote_level += 1;

                debug_log!("BlockQuote");
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };

                // If we're close to the chunk boundary, start the code block
                // on a fresh chunk so we don't end up force‑splitting the first
                // line of code mid‑word. Reserve space for the opening fence,
                // closing fence, and a little body headroom.
                const MIN_CODE_BODY_HEADROOM: usize = 4;
                let current_len = self.result.last().map(|s| s.len()).unwrap_or(0);
                let pending_prefix = self.pending_prefix_len();
                let header_len = 3 + lang.len(); // "```" + lang
                let closers_len = 3; // closing "```"
                let remaining_after_structure = self
                    .max_len
                    .saturating_sub(current_len + pending_prefix + header_len + closers_len);
                if current_len > 0 && remaining_after_structure < MIN_CODE_BODY_HEADROOM {
                    self.split_chunk();
                }

                self.output("```", false);
                self.output(&lang, true);
                self.add_new_line = true;
                self.stack.push(Descriptor::CodeBlock(lang));

                debug_log!("CodeBlock");
            }
            Tag::HtmlBlock => {
                debug_log!("HtmlBlock");
            }
            Tag::List(n) => {
                self.list = true;
                let extra_levels = if self.list_stack.is_empty() {
                    self.carry_list_indent_levels
                } else {
                    0
                };
                self.list_stack.push(ListState::new(n, extra_levels));
                self.carry_list_indent_levels = 0;

                debug_log!("List");
            }
            Tag::Item => {
                let has_extra_indent = self
                    .list_stack
                    .last()
                    .map(|s| s.extra_levels > 0)
                    .unwrap_or(false);
                if self.add_new_line
                    && (self.list_stack.len() > 1
                        || self.carry_list_indent_levels > 0
                        || has_extra_indent)
                {
                    // When starting a nested list item right after text, consume
                    // the pending newline instead of emitting an extra blank line.
                    self.flush_pending_prefix();
                } else {
                    self.new_line();
                }
                let (prefix, indent_len) = self.list_prefix();
                // Ensure the prefix fits; if not, split first.
                let pending_prefix = self.pending_prefix_len();
                let closers_len = self.closers_len(false);
                let current_len = self.result.last().map(|s| s.len()).unwrap_or(0);
                if current_len + pending_prefix + closers_len + prefix.len() >= self.max_len {
                    self.split_chunk();
                }
                self.flush_pending_prefix();
                let chunk_idx = self.result.len() - 1;
                let start = self.result[chunk_idx].len();
                self.result[chunk_idx].push_str(&prefix);
                let end = self.result[chunk_idx].len();

                if let Some(state) = self.list_stack.last_mut() {
                    if state.ordered {
                        state.items.push(ItemMarker {
                            chunk_idx,
                            start,
                            end,
                            indent_len,
                        });
                    }
                }
                self.after_list_prefix = true;

                debug_log!("Item");
            }
            Tag::FootnoteDefinition(_) => {
                debug_log!("FootnoteDefinition");
            }
            Tag::Table(_) => {
                debug_log!("Table");
            }
            Tag::TableHead => {
                debug_log!("TableHead");
            }
            Tag::TableRow => {
                debug_log!("TableRow");
            }
            Tag::TableCell => {
                debug_log!("TableCell");
            }
            Tag::Subscript => {
                debug_log!("Subscript");
            }
            Tag::Superscript => {
                debug_log!("Superscript");
            }
            Tag::Emphasis => {
                self.output("_", false);
                self.stack.push(Descriptor::Emphasis);

                debug_log!("Emphasis");
            }
            Tag::Strong => {
                self.output("*", false);
                self.stack.push(Descriptor::Strong);

                debug_log!("Strong");
            }
            Tag::Strikethrough => {
                self.output("~~", false);
                self.stack.push(Descriptor::Strikethrough);

                debug_log!("Strikethrough");
            }
            Tag::Link { dest_url, .. } => {
                assert!(self.link_dest_url.is_empty());

                self.link_dest_url = dest_url.to_string();

                debug_log!("Link");
            }
            Tag::Image { dest_url, .. } => {
                // Render images as a simple link placeholder: [Image](url)
                self.buffer.clear();
                self.buffer.push_str("[Image](");
                push_escaped(&mut self.buffer, &dest_url);
                self.buffer.push(')');

                let mut link = String::new();
                std::mem::swap(&mut self.buffer, &mut link);
                self.output_unbreakable(link.as_str(), false);
                self.buffer = link;
                self.buffer.clear();

                // Skip any nested alt-text events until the matching end tag to
                // avoid emitting the alt content (Telegram won't render it).
                self.skip_depth = 1;

                debug_log!("Image");
            }
            Tag::MetadataBlock(_) => {
                debug_log!("MetadataBlock");
            }
            Tag::DefinitionList => {
                debug_log!("DefinitionList");
            }
            Tag::DefinitionListTitle => {
                debug_log!("DefinitionListTitle");
            }
            Tag::DefinitionListDefinition => {
                debug_log!("DefinitionListDefinition");
            }
        }

        Ok(())
    }

    fn end_tag(&mut self, tag: TagEnd) -> anyhow::Result<()> {
        match tag {
            TagEnd::Paragraph => {
                self.add_new_line = true;

                debug_log!("EndParagraph");
            }
            TagEnd::Heading(level) => {
                match level {
                    HeadingLevel::H1 => self.output_closing("*", false),
                    HeadingLevel::H2 => self.output_closing("*", false),
                    HeadingLevel::H3 => self.output_closing("*", false),
                    HeadingLevel::H4 => self.output_closing("*", false),
                    HeadingLevel::H5 => self.output_closing("_", false),
                    HeadingLevel::H6 => self.output_closing("_", false),
                }
                self.add_new_line = true;

                debug_log!("EndHeading");
            }
            TagEnd::BlockQuote(_) => {
                self.add_new_line = true;
                self.quote_level -= 1;

                debug_log!("EndBlockQuote");
            }
            TagEnd::CodeBlock => {
                self.output_closing("```", false);
                self.add_new_line = true;
                self.close_descriptor(Descriptor::CodeBlock(String::new()))?;

                debug_log!("EndCodeBlock");
            }
            TagEnd::HtmlBlock => {
                debug_log!("EndHtmlBlock");
            }
            TagEnd::List(_) => {
                self.list = false;
                if let Some(state) = self.list_stack.pop() {
                    // If we just closed a single-item ordered list, consider the next
                    // top-level list as nested one level deeper (common in docs).
                    if state.ordered && state.items.len() == 1 && self.list_stack.is_empty() {
                        self.carry_list_indent_levels = 1;
                    } else if self.list_stack.is_empty() {
                        self.carry_list_indent_levels = 0;
                    }
                }
                self.add_new_line = true;
                self.after_list_prefix = false;

                debug_log!("EndList");
            }
            TagEnd::Item => {
                debug_log!("EndItem");
            }
            TagEnd::FootnoteDefinition => {
                debug_log!("EndFootnoteDefinition");
            }
            TagEnd::Table => {
                debug_log!("EndTable");
            }
            TagEnd::TableHead => {
                debug_log!("EndTableHead");
            }
            TagEnd::TableRow => {
                debug_log!("EndTableRow");
            }
            TagEnd::TableCell => {
                debug_log!("EndTableCell");
            }
            TagEnd::Subscript => {
                debug_log!("EndSubscript");
            }
            TagEnd::Superscript => {
                debug_log!("EndSuperscript");
            }
            TagEnd::Emphasis => {
                self.output_closing("_", false);
                self.close_descriptor(Descriptor::Emphasis)?;

                debug_log!("EndEmphasis");
            }
            TagEnd::Strong => {
                self.output_closing("*", false);
                self.close_descriptor(Descriptor::Strong)?;

                debug_log!("EndStrong");
            }
            TagEnd::Strikethrough => {
                self.output_closing("~~", false);
                self.close_descriptor(Descriptor::Strikethrough)?;
            }
            TagEnd::Link => {
                debug_log!("EndLink");
            }
            TagEnd::Image => {
                debug_log!("EndImage");
            }
            TagEnd::MetadataBlock(_) => {
                debug_log!("EndMetadataBlock");
            }
            TagEnd::DefinitionList => {
                debug_log!("EndDefinitionList");
            }
            TagEnd::DefinitionListTitle => {
                debug_log!("EndDefinitionListTitle");
            }
            TagEnd::DefinitionListDefinition => {
                debug_log!("EndDefinitionListDefinition");
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
        Descriptor::Strong => "*",
        Descriptor::Emphasis => "_",
        Descriptor::Strikethrough => "~~",
        Descriptor::Code => "`",
        Descriptor::CodeBlock(_) => "```",
    }
}

/// Escape Telegram MarkdownV2 control characters into the provided buffer.
fn push_escaped(out: &mut String, text: &str) {
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

#[derive(Debug, Default)]
struct ListState {
    ordered: bool,
    start: u64,
    items: Vec<ItemMarker>,
    extra_levels: usize,
}

impl ListState {
    fn new(n: Option<u64>, extra_levels: usize) -> Self {
        Self {
            ordered: n.is_some(),
            start: n.unwrap_or(1),
            items: Vec::new(),
            extra_levels,
        }
    }
}

#[derive(Debug, Clone)]
struct ItemMarker {
    chunk_idx: usize,
    start: usize,
    end: usize,
    indent_len: usize,
}
