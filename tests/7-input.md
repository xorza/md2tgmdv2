You do it by turning the LLM into a **multi‑step compressor** and *never* feeding it more text than fits at once.

Below is a concrete recipe, written so you can implement it.

---

## 1. What You’re Trying To Achieve

Given:

- A long conversation: a list like  
  `messages = [{role: "user"|"assistant", content: string}, …]`
- An LLM with a fixed context limit (token budget), e.g. 8k / 16k / 32k / 128k tokens.

You want to:

1. Produce a **summary of the whole conversation** that fits in the LLM’s context, and/or  
2. Keep chatting with the user while the LLM still has access to relevant past information, **even after** the raw history no longer fits.

You cannot just send the whole conversation; instead you:

- **Split** it into chunks that fit;
- **Summarize** each chunk using the LLM;
- Optionally **summarize those summaries** (hierarchical);
- Maintain a **small “memory”** object that represents the past.

---

## 2. Set Explicit Token Budgets

You must pick explicit numbers, not guess.

Assume:

- `MODEL_CONTEXT_TOKENS` = max tokens your LLM can accept (say 16,000).

Define:

- `MAX_CHUNK_TOKENS` — maximum tokens of *conversation text* you send in a *single summarization* call.  
  Example: `MAX_CHUNK_TOKENS = 3_000`.

- `TARGET_CHUNK_SUMMARY_TOKENS` — target size of each first‑level summary.  
  Example: `300`.

- `TARGET_GROUP_SUMMARY_TOKENS` — target size of each higher‑level summary.  
  Example: `400`.

- `TARGET_GLOBAL_SUMMARY_TOKENS` — target size of final conversation summary.  
  Example: `800–1_200`.

For ongoing chat, also:

- `MEMORY_TOKEN_LIMIT` — max size of long‑term memory.  
  Example: `600`.

- `RECENT_WINDOW_TOKEN_LIMIT` — how many tokens of raw “recent” conversation you aim to keep.  
  Example: `3_000`.

Use the model’s tokenizer (e.g. `tiktoken`) to write:

```text
token_count(text) -> approximate token count
```

Everything else will rely on this.

---

## 3. Offline: Summarize an Existing Long Conversation

### 3.1. Step 1 – Chunk the Conversation

Input:

```text
messages = [
  {role: "user", content: "..."},
  {role: "assistant", content: "..."},
  ...
]
```

Goal: `chunks[]`, where each `chunk` is a list of messages whose total tokens ≤ `MAX_CHUNK_TOKENS`.

Pseudocode:

```pseudo
function chunk_messages(messages, max_chunk_tokens):
    chunks = []
    current_chunk = []
    current_tokens = 0

    for msg in messages:
        text = msg.role + ": " + msg.content
        t = token_count(text)

        if t > max_chunk_tokens:
            # Edge case: single message is longer than allowed.
            # Summarize this one separately instead of putting it raw in a chunk.
            summary = summarize_single_long_message_with_llm(msg)
            chunks.append([
                {role: "assistant", content: "(summary of long message) " + summary}
            ])
            continue

        if current_tokens + t > max_chunk_tokens and current_chunk not empty:
            chunks.append(current_chunk)
            current_chunk = [msg]
            current_tokens = t
        else:
            current_chunk.append(msg)
            current_tokens += t

    if current_chunk not empty:
        chunks.append(current_chunk)

    return chunks
```

Notes:

- You always cut at **message boundaries**.
- `summarize_single_long_message_with_llm` is just a one‑off “summarize this text” call.

---

### 3.2. Step 2 – Summarize Each Chunk with the LLM

For each chunk, call the LLM with a strict summarization prompt.

#### Prompt template (per chunk, copy‑paste):

> You are summarizing a segment of a long conversation between a user and an assistant.  
>  
> Your goal is to produce a short but highly informative summary that can replace the raw messages in future prompts.  
>  
> **INCLUDE (only if present and important):**  
> - Main user questions, tasks, and goals in this segment  
> - Important facts, constraints, and preferences the user states (deadlines, environment, skill level, likes/dislikes, etc.)  
> - Key explanations, designs, solution ideas, and reasoning from the assistant (described concisely in words; avoid large code blocks)  
> - Any decisions made, final answers given, or conclusions reached  
> - Any explicit open questions or TODO items mentioned  
>  
> **EXCLUDE OR MINIMIZE:**  
> - Greetings, small talk, and filler conversation  
> - Repetitive text that adds no new information  
> - Very low‑level intermediate steps or details unlikely to matter later  
>  
> **OUTPUT FORMAT (plain text, no JSON, no code fences):**  
> - Topics: bullet list of the main topics discussed.  
> - User Goals: bullet list of what the user wanted or asked for in this segment.  
> - Key Facts: bullet list of important facts, constraints, or preferences.  
> - Assistant Actions: bullet list describing what the assistant did (explained X, proposed Y, wrote code for Z, etc.).  
> - Decisions / Outcomes: bullet list of any conclusions, solutions, or decisions reached.  
> - Open Questions / TODOs: bullet list of unresolved issues or future steps.  
>  
> **LENGTH LIMIT:**  
> - Maximum 300 tokens. Be concise and information‑dense.  
>  
> Now summarize the following conversation segment:  
>  
> [BEGIN SEGMENT]  
> <insert this chunk’s messages, each prefixed with “user:” or “assistant:”>  
> [END SEGMENT]
