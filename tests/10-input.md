You summarize a conversation that doesn’t fit an LLM’s token budget by using a **bounded, multi-pass compression pipeline**. The LLM never sees the full transcript at once; instead it sees **chunks**, then **summaries of chunks**, and you maintain a **small long‑term memory** for future turns.

Below is a very specific, verbose, implementable approach.

---

## 1) Decide your budgets (exact numbers you enforce)

You need the model’s context size:

- `MODEL_CONTEXT_TOKENS` (e.g., 16,000)

Reserve tokens for things that are *not* the transcript:

- `RESERVED_OUTPUT_TOKENS` (space for the model’s reply): e.g., 1,200–2,000
- `RESERVED_OVERHEAD_TOKENS` (system prompt + tool schemas + wrappers): e.g., 600–1,500

Compute your safe maximum input:

- `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example:

- `MODEL_CONTEXT_TOKENS = 16000`
- `RESERVED_OUTPUT_TOKENS = 1500`
- `RESERVED_OVERHEAD_TOKENS = 800`
- `MAX_INPUT_TOKENS = 13700`

Now define summarization budgets:

- `MAX_CHUNK_TOKENS` (raw transcript tokens per summarization call): e.g., 3000
- `TARGET_CHUNK_SUMMARY_TOKENS`: e.g., 300–450
- `TARGET_GROUP_SUMMARY_TOKENS`: e.g., 400–600
- `TARGET_GLOBAL_SUMMARY_TOKENS`: e.g., 800–1200

If you’re doing *ongoing chat*:

- `MEMORY_TOKEN_LIMIT`: e.g., 600
- `RECENT_WINDOW_TOKEN_LIMIT`: e.g., 3000

**Non-negotiable:** implement `token_count(text)` using the model’s real tokenizer (don’t approximate by characters).

---

## 2) Store conversation in a structured format

Use:

```json
[
  {"id": 1, "role": "user", "content": "...", "timestamp": "..."},
  {"id": 2, "role": "assistant", "content": "...", "timestamp": "..."}
]
```

Why this matters:
- You can chunk reliably at message boundaries.
- You can track “chunk summary #7 covers messages 120–145”.
- You can re-summarize just one segment later.

---

## 3) Step A: Chunk the conversation (token-aware, message boundaries only)

Goal: create chunks of messages where each chunk’s text fits into `MAX_CHUNK_TOKENS`.

### Chunking pseudocode

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current_chunk = []
    current_tokens = 0

    for msg in messages:
        msg_text = msg.role + ": " + msg.content
        t = token_count(msg_text)

        if t > MAX_CHUNK_TOKENS:
            # Oversized single message case (huge paste/log).
            # Option 1 (recommended): summarize this message alone.
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

**Important edge case:** a single message bigger than your chunk limit must be handled separately (summarize it or split it by paragraphs first), or the pipeline breaks.

---

## 4) Step B: Summarize each chunk with a strict schema (first-level summaries)

The most common failure is asking “summarize this” and getting a vague narrative. You want summaries that preserve:
- goals
- constraints
- decisions
- TODOs
- important numbers/commands/paths/errors

### Chunk summarization prompt (copy/paste)

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
> MUST MINIMIZE: greetings, filler, repetition, incidental details.  
>  
> OUTPUT FORMAT (plain text, no JSON, no code fences):  
> - Topics:  
> - User Goals:  
> - Key Facts / Constraints:  
> - Assistant Actions / Suggestions:  
> - Decisions / Outcomes:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 350 tokens. Preserve important numbers, commands, filenames, and errors.  
>  
> [BEGIN SEGMENT]  
> <paste chunk messages with "user:" / "assistant:" labels>  
> [END SEGMENT]

Store output:

```json
{
  "chunk_id": 7,
  "covers_message_ids": [120,121,122],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

Now you have N chunk summaries.

---

## 5) Step C: Summarize the summaries (hierarchical compression)

If N chunk summaries still don’t fit into your desired context, compress again.

### 5.1 Group chunk summaries into batches

If each chunk summary is ~350 tokens and you can feed ~3000 tokens per call, group size ~8 is safe:

- `group_size = 8` (8 × 350 = 2800 tokens)

```pseudo
function group_items(items, group_size):
    groups = []
    for i in range(0, len(items), group_size):
        groups.append(items[i : i + group_size])
    return groups
```

### 5.2 Group compression prompt (copy/paste)

> You are compressing multiple chunk summaries into one higher-level summary.  
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

Run per group → you get second-level summaries.  
If still too large, repeat: group second-level summaries, summarize again, until you produce a single global summary under `TARGET_GLOBAL_SUMMARY_TOKENS`.

---

## 6) Step D: Convert the global summary into bounded long-term memory

Now create a memory object you can inject into future prompts.

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
> HARD LIMIT: ≤ 600 tokens. Drop least durable details first if needed.  
>  
> [BEGIN SUMMARY]  
> <global summary>  
> [END SUMMARY]

Store as `long_term_memory`.

---

## 7) Ongoing chat: rolling memory updates (so you never overflow)

Maintain:
- `long_term_memory` (≤ `MEMORY_TOKEN_LIMIT`)
- `recent_messages` (raw last ~`RECENT_WINDOW_TOKEN_LIMIT` tokens)

### Trigger rule (explicit)

Before each LLM call, compute:

`prompt_tokens = token_count(system + long_term_memory + recent_messages + new_user_message)`

If:

`prompt_tokens > MAX_INPUT_TOKENS * 0.9`

then:
1) select old part of `recent_messages` (keep last ~8 turns raw)  
2) summarize that segment using the chunk summarizer  
3) merge that segment summary into `long_term_memory` using an update prompt  
4) delete summarized raw messages

### Update-memory prompt (copy/paste)

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
> - removing low-value/obsolete details to stay under the size limit  
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

---

## 8) Optional but best: retrieval over chunk summaries

If you need higher detail recall:
- store chunk summaries
- embed them
- on each new user question, retrieve top-K relevant summaries
- include only those summaries + memory + recent window in the prompt

This gives better “memory” without increasing token usage much.

---

If you tell me:
1) which model/context size you’re using, and  
2) whether you need one-time summarization or an ongoing chat memory system,  
I can propose specific default values (chunk size, group size, trigger thresholds) tailored to that setup.
