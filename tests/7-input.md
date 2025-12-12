You summarize a conversation that doesn’t fit an LLM’s token budget by building a **repeatable compression pipeline** that (a) never exceeds the context limit and (b) preserves the *right* information (goals, constraints, decisions, TODOs) instead of wasting tokens on narrative.

The practical solution is a **hierarchical summarization + rolling memory** approach:

1. **Chunk** the transcript into pieces that fit.
2. **Summarize each chunk** with a strict schema.
3. If still too big, **summarize the summaries** (repeat until it fits).
4. Convert the final summary into a **bounded long‑term memory** object.
5. For ongoing chat, **incrementally summarize** older turns into memory and keep only a small “recent window” raw.

Below is a very specific, verbose, implementable guide.

---

## 1) Establish hard budgets (non‑negotiable)

You must pick explicit numbers based on your model.

### 1.1 Context math

Let:

- `MODEL_CONTEXT_TOKENS` = model max context (e.g., 16,000)
- `RESERVED_OUTPUT_TOKENS` = how many tokens you want for the model’s reply (e.g., 1,200)
- `RESERVED_OVERHEAD_TOKENS` = system prompt + tool schemas + wrappers (e.g., 800)

Then:

- `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example:

- `MODEL_CONTEXT_TOKENS = 16000`
- `RESERVED_OUTPUT_TOKENS = 1500`
- `RESERVED_OVERHEAD_TOKENS = 800`
- `MAX_INPUT_TOKENS = 13700`

### 1.2 Summarization budgets

Choose:

- `MAX_CHUNK_TOKENS` (input to a summarization call): **2,500–4,000** (e.g., 3,000)
- `TARGET_CHUNK_SUMMARY_TOKENS`: **250–500** (e.g., 350)
- `TARGET_GROUP_SUMMARY_TOKENS`: **300–600** (e.g., 450)
- `TARGET_GLOBAL_SUMMARY_TOKENS`: **600–1,200** (e.g., 900)

For ongoing chat:

- `MEMORY_TOKEN_LIMIT`: **400–800** (e.g., 600)
- `RECENT_WINDOW_TOKEN_LIMIT`: **2,000–4,000** (e.g., 3,000)

### 1.3 Use real token counting

You need a function like:

- `token_count(text) -> int`

Implement it with the model’s tokenizer (don’t approximate by characters).

---

## 2) Store the conversation in a usable structure

Your raw data should look like:

```json
[
  {"id": 1, "role": "user", "content": "...", "ts": "..."},
  {"id": 2, "role": "assistant", "content": "...", "ts": "..."},
  ...
]
```

Why IDs matter:

- You can track “chunk #12 covers messages 300–341”
- You can re-summarize only a portion later
- You can debug summary errors

---

## 3) Step A — Chunk the conversation (token‑aware, message‑boundary safe)

You must chunk at message boundaries.

### 3.1 Chunking algorithm

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current_chunk = []
    current_tokens = 0

    for msg in messages:
        msg_text = msg.role + ": " + msg.content
        t = token_count(msg_text)

        if t > MAX_CHUNK_TOKENS:
            # Oversized single message:
            # summarize it alone or split it by paragraphs first
            short = summarize_single_message(msg)
            replacement = {role: "assistant", content: "(summary of oversized message) " + short}

            if current_chunk not empty:
                chunks.append(current_chunk)
                current_chunk = []
                current_tokens = 0

            chunks.append([replacement])
            continue

        if current_tokens + t > MAX_CHUNK_TOKENS and current_chunk not empty:
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

### 3.2 Oversized single-message handling (important)

If a single message is huge (pasted logs, large code blocks), you have two safe options:

- **Option 1 (recommended):** Run a “summarize this message” call and replace it with the summary.
- **Option 2:** Split that message into paragraphs/sections, summarize each, then merge.

If you don’t do this, chunking breaks.

---

## 4) Step B — Summarize each chunk with a strict schema

The quality of your whole pipeline depends on the *schema*. Without it, you’ll lose constraints and decisions.

### 4.1 Chunk summarization prompt (copy‑paste)

Use this for every chunk:

> You are summarizing a segment of a long user–assistant conversation.  
> PURPOSE: Produce a compact, information-dense summary that can replace the raw messages in future prompts.  
>  
> MUST CAPTURE (if present):  
> - User goals/questions/tasks in this segment  
> - Key facts and constraints (numbers, deadlines, environment, versions, file paths, commands)  
> - Assistant’s substantive contributions (plans, reasoning, designs; describe changes instead of pasting long code)  
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

### 4.2 Store chunk summaries with metadata

Store something like:

```json
{
  "chunk_id": 7,
  "covers_message_ids": [120,121,122,...],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

Now you have first-level summaries.

---

## 5) Step C — Summarize the summaries (hierarchical compression)

If you have many chunk summaries, they may still exceed your final budget.

### 5.1 Grouping

If chunk summaries are ~350 tokens and your `MAX_CHUNK_TOKENS` is ~3000, then:

- `group_size ≈ 8` (8 × 350 = 2800 tokens input) is often safe.

```pseudo
function group_items(items, group_size):
    groups = []
    for i in range(0, len(items), group_size):
        groups.append(items[i : i + group_size])
    return groups
```

### 5.2 Group compression prompt (copy‑paste)

> You are compressing multiple chunk summaries of a conversation into one higher-level summary.  
>  
> GOAL: Merge redundancy while preserving durable, decision-relevant information:  
> - stable user profile/preferences/constraints  
> - major solution approaches and why they were chosen  
> - key decisions/outcomes  
> - long-running threads and status  
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

Run this per group → you get second-level summaries.

### 5.3 Repeat until it fits

If second-level summaries still too large:

- group them again
- summarize again

Stop when you have:

- 1 global summary (preferred), or
- a small set that fits your target input.

This is the “summary tree”:

**raw transcript → chunk summaries → group summaries → global summary**

---

## 6) Step D — Convert global summary into bounded long‑term memory

Now create a compact memory object designed for future prompt injection.

### 6.1 Memory creation prompt (copy‑paste)

> Convert the following conversation summary into long-term memory for future turns.  
> Keep only information likely to matter later.  
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
> HARD LIMIT: ≤ 600 tokens. If you must drop details, drop the least durable ones first.  
>  
> [BEGIN SUMMARY]  
> <global summary>  
> [END SUMMARY]

Store this as `long_term_memory`.

---

## 7) Ongoing chat: rolling memory updates (the “never overflow” loop)

For a live conversation, maintain:

- `long_term_memory` (≤ 600 tokens)
- `recent_messages` (raw recent window)
- optional: chunk summaries for retrieval

### 7.1 Trigger rule (very explicit)

Before every model call, compute:

`prompt_tokens = token_count(system + long_term_memory + recent_messages + new_user_message)`

If:

- `prompt_tokens > MAX_INPUT_TOKENS * 0.9`

then compress.

### 7.2 What to compress

Keep last `K` turns raw (e.g., 8). Summarize older:

```pseudo
function select_old_segment(recent_messages, keep_last=8, max_tokens=2000):
    if len(recent_messages) <= keep_last:
        return []
    candidates = recent_messages[0 : -keep_last]
    return trim_to_token_limit(candidates, max_tokens)
```

### 7.3 Merge into memory (update prompt)

> You maintain a bounded long-term memory of a conversation.  
>  
> CURRENT MEMORY:  
> <existing memory>  
>  
> NEW INFORMATION (summary of older recent turns):  
> <segment summary>  
>  
> Update memory by:  
> - adding important persistent info  
> - correcting anything that changed  
> - removing low-value/obsolete details to stay under size limit  
>  
> OUTPUT the UPDATED MEMORY ONLY in this format:  
> - User Profile:  
> - Preferences:  
> - Constraints / Environment:  
> - Projects / Status:  
> - Key Decisions / Rationale:  
> - Open Questions / TODOs:  
>  
> HARD LIMIT: ≤ 600 tokens.

Then:

- Replace `long_term_memory` with updated version
- Delete those old raw messages from `recent_messages`

Now you’re safely under budget again.

---

## 8) Optional but best: retrieval (semantic search over chunk summaries)

A single memory blob can drop details. The best practical setup is:

1. Store all chunk summaries.
2. Compute embeddings for each chunk summary.
3. On each new user request:
   - embed the request
   - retrieve top‑K relevant chunk summaries
4. Build the prompt from:
   - system instructions
   - long_term_memory
   - top‑K retrieved summaries
   - recent raw messages
   - new user message

This yields higher accuracy without exceeding context.

---

## 9) Common pitfalls and fixes

1) **Summaries get too vague**  
Fix: enforce schema and require “Key Facts / Constraints” and “Decisions” and “TODOs”.

2) **Numbers / commands / error messages lost**  
Fix: explicitly instruct “preserve all numbers, commands, filenames, errors”.

3) **Memory grows indefinitely**  
Fix: strict `MEMORY_TOKEN_LIMIT` and “drop least durable details first”.

4) **You overflow anyway**  
Fix: start summarizing at 60–70% usage, not at 95–100%.

5) **LLM hallucinates missing history**  
Fix: system instruction: “If not in memory or retrieved summaries, ask the user for details.”

---

If you tell me (a) which model/context size you’re using and (b) whether you want offline summarization or live rolling memory, I can propose exact default numbers (chunk size, group size, trigger thresholds) that typically work well.