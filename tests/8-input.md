You summarize a conversation that does **not** fit the token budget by using the LLM in **multiple bounded passes** and by keeping a **rolling memory** so you never need to resend the full transcript.

The practical pattern is:

1. **Chunk** the transcript into pieces that fit.
2. **Summarize each chunk** with a strict schema (so you don’t lose constraints/decisions).
3. If still too big, **summarize the summaries** (hierarchical compression) until it fits.
4. Convert the final result into a **small “long‑term memory”** you can carry forward.
5. In ongoing chats, periodically summarize older turns into memory and keep only a **recent raw window**.

Below is a very specific, implementable guide.

---

## 1) Establish hard budgets (numbers you enforce)

You need the model’s maximum context size:

- `MODEL_CONTEXT_TOKENS` (e.g., 8k / 16k / 32k / 128k)

Then reserve capacity so you don’t overflow:

- `RESERVED_OUTPUT_TOKENS` (space for the model’s answer): 800–2000
- `RESERVED_OVERHEAD_TOKENS` (system prompt + formatting + tools): 500–1500

Compute:

- `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example (16k model):

- `MODEL_CONTEXT_TOKENS = 16000`
- `RESERVED_OUTPUT_TOKENS = 1500`
- `RESERVED_OVERHEAD_TOKENS = 800`
- `MAX_INPUT_TOKENS = 13700`

Now set summarization parameters:

- `MAX_CHUNK_TOKENS` (input per summarization call): 2500–4000 (e.g., 3000)
- `TARGET_CHUNK_SUMMARY_TOKENS`: 250–500 (e.g., 350)
- `TARGET_GROUP_SUMMARY_TOKENS`: 300–600 (e.g., 450)
- `TARGET_GLOBAL_SUMMARY_TOKENS`: 600–1200 (e.g., 900)

For ongoing chat:

- `MEMORY_TOKEN_LIMIT`: 400–800 (e.g., 600)
- `RECENT_WINDOW_TOKEN_LIMIT`: 2000–4000 (e.g., 3000)

**Non-negotiable:** implement `token_count(text)` using the model’s tokenizer (don’t approximate by characters).

---

## 2) Store your conversation in a structured format

Use:

```json
[
  {"id": 1, "role": "user", "content": "...", "ts": "..."},
  {"id": 2, "role": "assistant", "content": "...", "ts": "..."}
]
```

Why:

- You can track which messages are covered by which chunk summary.
- You can re-summarize only portions later.
- You can do retrieval later (optional but powerful).

---

## 3) Step A — Token-aware chunking (message boundaries only)

Goal: produce `chunks[]` where each chunk fits into `MAX_CHUNK_TOKENS`.

Pseudocode:

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current = []
    current_tokens = 0

    for msg in messages:
        text = msg.role + ": " + msg.content
        t = token_count(text)

        if t > MAX_CHUNK_TOKENS:
            # Oversized single message (huge paste).
            # Summarize it alone first, replace it with a short “replacement message”.
            short = summarize_single_message(msg)
            replacement = {role: "assistant", content: "(summary of oversized message) " + short}

            if current not empty:
                chunks.append(current)
                current = []
                current_tokens = 0

            chunks.append([replacement])
            continue

        if current_tokens + t > MAX_CHUNK_TOKENS and current not empty:
            chunks.append(current)
            current = [msg]
            current_tokens = t
        else:
            current.append(msg)
            current_tokens += t

    if current not empty:
        chunks.append(current)

    return chunks
```

Important details:

- Never split in the middle of a message unless you implement special logic.
- Huge single messages must be handled separately.

---

## 4) Step B — Summarize each chunk with a strict schema

A vague “summarize this” prompt loses constraints and decisions. Use a fixed schema.

### Chunk summary prompt (copy/paste)

> You are summarizing a segment of a long user–assistant conversation.  
> PURPOSE: Produce a compact, information-dense summary that can replace the raw messages in future prompts.  
>  
> MUST CAPTURE (if present):  
> - User goals/questions/tasks in this segment  
> - Key facts and constraints (numbers, deadlines, environment, versions, file paths, commands, error messages)  
> - Assistant’s substantive contributions (plans, reasoning, designs; describe code changes instead of pasting long code)  
> - Decisions/outcomes reached (what was chosen and why)  
> - Open questions / TODOs created or still unresolved  
>  
> MUST MINIMIZE: greetings, filler, repetition, incidental detail.  
>  
> OUTPUT FORMAT (plain text, no JSON, no code fences):  
> - Topics:  
> - User Goals:  
> - Key Facts / Constraints:  
> - Assistant Actions / Suggestions:  
> - Decisions / Outcomes:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 350 tokens. Preserve all important numbers, commands, filenames, and errors.  
>  
> [BEGIN SEGMENT]  
> <paste the chunk messages with "user:" / "assistant:" labels>  
> [END SEGMENT]

Store results like:

```json
{
  "chunk_id": 7,
  "covers_message_ids": [120, 121, 122],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

Now you have **first-level summaries**.

---

## 5) Step C — Summarize the summaries (hierarchical compression)

If you have many chunk summaries, compress again.

### 5.1 Group chunk summaries into batches that fit

If each chunk summary is ~350 tokens and `MAX_CHUNK_TOKENS` ≈ 3000, pick:

- `group_size = 8` (8 × 350 = 2800 tokens, leaving room for instructions)

```pseudo
function group_items(items, group_size):
    groups = []
    for i in range(0, len(items), group_size):
        groups.append(items[i : i + group_size])
    return groups
```

### 5.2 Prompt to compress multiple summaries (copy/paste)

> You are compressing multiple chunk summaries into a higher-level summary.  
>  
> GOAL: Merge redundancy while preserving durable, decision-relevant information:  
> - stable user profile/preferences/constraints  
> - major solution approaches and why they were chosen  
> - key decisions/outcomes  
> - long-running threads and current status  
> - open questions/TODOs  
>  
> OUTPUT FORMAT:  
> - Themes:  
> - Timeline / Phases:  
> - Persistent Facts / Preferences:  
> - Key Decisions / Outcomes:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 450 tokens. Remove repetition aggressively.  
>  
> [BEGIN INPUT SUMMARIES]  
> <paste 8–10 chunk summaries in chronological order>  
> [END INPUT SUMMARIES]

Run for each group → get **second-level summaries**.

If second-level is still too big:

- Group second-level summaries
- Compress again
- Repeat until you get a **global summary** under `TARGET_GLOBAL_SUMMARY_TOKENS`.

This forms a tree:

**raw transcript → chunk summaries → group summaries → global summary**

---

## 6) Step D — Convert the global summary into bounded long-term memory

Now produce a reusable memory object (small enough to include in every future prompt).

### Memory creation prompt (copy/paste)

> Convert the following conversation summary into long-term memory for future turns. Keep only information likely to matter later.  
>  
> Include:  
> - User Profile (skills, preferences, communication style)  
> - Constraints / Environment (OS, tools, versions, repo structure)  
> - Projects / Status (what’s being built, current progress)  
> - Key Decisions + short rationale  
> - Open Questions / TODOs  
>  
> Output format (plain text):  
> - User Profile:  
> - Preferences:  
> - Constraints / Environment:  
> - Projects / Status:  
> - Key Decisions / Rationale:  
> - Open Questions / TODOs:  
>  
> HARD LIMIT: ≤ 600 tokens. Drop the least durable details first if needed.  
>  
> [BEGIN SUMMARY]  
> <global summary>  
> [END SUMMARY]

Store as `long_term_memory`.

---

## 7) Ongoing chat: rolling memory updates (so you never overflow)

Maintain:

- `long_term_memory` (≤ `MEMORY_TOKEN_LIMIT`)
- `recent_messages` (raw, last ~`RECENT_WINDOW_TOKEN_LIMIT` tokens)

### Trigger rule

Before each model call, compute:

`prompt_tokens = token_count(system + long_term_memory + recent_messages + new_user_message)`

If:

`prompt_tokens > MAX_INPUT_TOKENS * 0.9`

then compress older `recent_messages` into memory.

### What to compress

Keep last K turns raw, summarize the rest:

```pseudo
function select_old_segment(recent_messages, keep_last=8, max_tokens=2000):
    if len(recent_messages) <= keep_last:
        return []
    candidates = recent_messages[0 : -keep_last]
    return trim_to_token_limit(candidates, max_tokens)
```

Summarize `old_segment` with the same chunk summarizer prompt.

### Merge into memory with an update prompt

> CURRENT MEMORY: <memory>  
> NEW INFO: <segment summary>  
> Update memory by adding important persistent info, correcting changes, and dropping low-value details to stay ≤ 600 tokens.  
> Output UPDATED MEMORY ONLY in the standard sections.

Then delete those old raw messages.

---

## 8) Optional but best: retrieval over summaries

A single memory blob can lose detail. The best scalable design is:

- Store chunk summaries + embeddings
- On each new query:
  - embed the query
  - retrieve top-K relevant chunk summaries
- Prompt includes:
  - long_term_memory
  - top-K retrieved summaries
  - recent raw messages

This gives high recall without blowing context.

---

If you tell me which model/context size you’re using and whether this is (a) one-time summarization or (b) an ongoing chat system, I can suggest concrete defaults (chunk size, group size, trigger thresholds) that work well for that exact setup.
