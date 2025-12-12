# md2tgmdv2

Telegram MarkdownV2 renderer used to post LLM output into a Telegram bot.

## Status
- Early, unstable, API may change without notice.
- Some Markdown constructs are intentionally limited; tables are skipped. Images are rendered as `[Image](url)` placeholders.

## What it does
- Parses Markdown and emits Telegram-safe MarkdownV2, splitting messages to respect Telegram’s 4096 character limit.
- Escapes characters that Telegram treats as formatting control.
- Keeps code blocks and links intact while splitting at safe boundaries.

## What is missing (so far)
- Images, tables, and other rich blocks are not rendered.
- No streaming interface; everything is processed in-memory.
- Limited language detection for fenced code blocks.

## Quick start
```rust
use md2tgmdv2::Converter;

fn main() -> anyhow::Result<()> {
    let markdown = "Hello **world** from OpenAI!";
    let chunks = Converter::default().go(markdown)?;
    for chunk in chunks {
        // send each chunk to Telegram Bot API
        println!("{chunk}");
    }
    Ok(())
}
```

## How I use it
- I run this library to post-clean and split OpenAI responses before sending them via my Telegram bot.

## Testing
```sh
cargo test
```

If you hit a case that breaks or should be supported, please open an issue or PR. The surface area is small and changes fast right now.
