rust code:

```rust
function chunk_messages(messages, max_chunk_tokens):
    chunks = []
    current_chunk = []
    current_tokens = 0

    for msg in messages:
        text = msg.role + ": " + msg.content
        t = token_count(text)

    if current_chunk not empty:
        chunks.append(current_chunk)

    return chunks
```

Notes:

- You always cut at **message boundaries**.
- `summarize_single_long_message_with_llm` is just a one‑off “summarize this text” call.
