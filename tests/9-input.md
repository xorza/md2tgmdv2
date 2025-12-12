To summarize a conversation with an LLM when the full transcript does **not** fit your token budget, you need a **controlled, multi-pass summarization pipeline** plus (if the conversation continues) a **rolling memory** strategy.

The key principle is:

> **Never try to send the whole conversation.**  
> Instead, send **bounded chunks**, summarize them, and keep only compact artifacts that can be reused.

Below is a very specific, verbose, implementable approach.

---

## 1) Decide Your Token Budgets (Hard Numbers)

You need the model’s context window:

- `MODEL_CONTEXT_TOKENS` (example: 16,000)

You must reserve space for:

1) **The model’s output**  
- `RESERVED_OUTPUT_TOKENS` (example: 1,200–2,000)

2) **Prompt overhead** (system instructions, formatting, tools)  
- `RESERVED_OVERHEAD_TOKENS` (example: 500–1,500)

Compute a safe max input budget:

- `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example:

- `MODEL_CONTEXT_TOKENS = 16000`  
- `RESERVED_OUTPUT_TOKENS = 1500`  
- `RESERVED_OVERHEAD_TOKENS = 800`  
- `MAX_INPUT_TOKENS = 13700`

Now choose summarization budgets:

- `MAX_CHUNK_TOKENS` (input per summarization call): 2,500–4,000 (example: 3,000)
- `TARGET_CHUNK_SUMMARY_TOKENS`: 250–500 (example: 350)
- `TARGET_GROUP_SUMMARY_TOKENS`: 300–600 (example: 450)
- `TARGET_GLOBAL_SUMMARY_TOKENS`: 600–1,200 (example: 900)

If you’re doing ongoing chat:

- `MEMORY_TOKEN_LIMIT`: 400–800 (example: 600)
- `RECENT_WINDOW_TOKEN_LIMIT`: 2,000–4,000 (example: 3,000)

### Mandatory: Real token counting
Implement `token_count(text)` using the model’s tokenizer (don’t estimate by characters).

---

## 2) Store the Conversation in a Structured Form

Represent each message as:

```json
[
  {"id": 1, "role": "user", "content": "...", "timestamp": "..."},
  {"id": 2, "role": "assistant", "content": "...", "timestamp": "..."}
]
```

Why this matters:

- You can chunk reliably.
- You can track what each summary covers.
- You can debug missing facts (“which chunk contained that?”).

---

## 3) Step A — Chunk the Conversation (Token-aware, Message-boundary Safe)

Goal: split the conversation into chunks where each chunk fits under `MAX_CHUNK_TOKENS`.

### Chunking algorithm (pseudocode)

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current_chunk = []
    current_tokens = 0

    for msg in messages:
        msg_text = msg.role + ": " + msg.content
        t = token_count(msg_text)

        # Edge case: a single message is too big
        if t > MAX_CHUNK_TOKENS:
            short = summarize_single_message(msg)  # one-off LLM call
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

### Oversized message handling (important)
If a user pastes a huge log or file, you must:
- summarize that one message separately, or
- split it into paragraphs and summarize paragraphs.

Otherwise your chunker will fail.

---

## 4) Step B — Summarize Each Chunk With a Strict Schema (First-level Summaries)

If you just say “summarize,” you lose constraints, decisions, and TODOs. Force structure.

### Chunk summary prompt (copy-paste)

> You are summarizing a segment of a long user–assistant conversation.  
> PURPOSE: Produce a compact, information-dense summary that can replace the raw messages in future prompts.  
>  
> MUST CAPTURE (if present):  
> - User goals/questions/tasks in this segment  
> - Key facts and constraints (numbers, deadlines, environment, versions, file paths, commands, error messages)  
> - Assistant’s substantive contributions (plans, reasoning, designs; describe changes instead of pasting long code)  
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
> HARD LENGTH LIMIT: ≤ 350 tokens. Preserve all important numbers, commands, filenames, and errors.  
>  
> [BEGIN SEGMENT]  
> <paste the chunk messages with "user:" / "assistant:" labels>  
> [END SEGMENT]

Store:

```json
{
  "chunk_id": 7,
  "covers_message_ids": [120, 121, 122],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

---

## 5) Step C — Summarize the Summaries (Hierarchical Compression)

If you have many chunk summaries and they still don’t fit, compress them again.

### 5.1 Group summaries into batches

If chunk summaries are ~350 tokens and you can fit ~3000 tokens, choose:

- `group_size ≈ 8` (8 × 350 = 2800)

```pseudo
function group_items(items, group_size):
    groups = []
    for i in range(0, len(items), group_size):
        groups.append(items[i : i + group_size])
    return groups
```

### 5.2 Prompt to compress multiple summaries

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

Run per group → you get “second-level summaries”.

Repeat (group again, summarize again) until you get a final global summary under `TARGET_GLOBAL_SUMMARY_TOKENS`.

This forms a summary tree:

**raw transcript → chunk summaries → group summaries → global summary**

---

## 6) Step D — Convert Global Summary Into Long-term Memory (Bounded, Reusable)

Now create a memory object you can include in future prompts.

### Memory creation prompt (copy-paste)

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

## 7) Ongoing Chat: Rolling Memory Updates (Never Overflow)

Maintain:

- `long_term_memory` (≤ `MEMORY_TOKEN_LIMIT`)
- `recent_messages` (raw recent window)

Before each model call, compute:

`prompt_tokens = token_count(system + long_term_memory + recent_messages + new_user_msg)`

If:

`prompt_tokens > MAX_INPUT_TOKENS * 0.9`

then:

1) select the oldest part of `recent_messages` (keep last ~8 turns)
2) summarize that old segment
3) merge it into `long_term_memory` using an update-memory prompt
4) remove those raw messages from `recent_messages`

### Update-memory prompt (copy-paste)

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

---

## 8) Optional (Best Practice): Retrieval Over Chunk Summaries

A single memory blob can drop detail. Best scalable approach:

1) Store chunk summaries + embeddings  
2) On each new query, retrieve top-K relevant summaries  
3) Prompt includes:
- long_term_memory
- retrieved top-K summaries
- recent raw messages

This increases “recall” without increasing context much.

---

If you tell me which exact model/context size you’re using and whether you’re building an app (API) or doing this manually in a chat UI, I can propose concrete default numbers (chunk size, group size, trigger thresholds) and a minimal working implementation outline.
