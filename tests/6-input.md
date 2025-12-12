You summarize a conversation that won’t fit in an LLM’s token budget by building a **controlled compression pipeline**. The key is to **summarize in chunks** (each chunk fits), then optionally **summarize the summaries** (hierarchy), and maintain a **bounded long‑term memory** that you can carry forward into future prompts.

Below is a very specific, verbose, implementable approach.

---

## 1) Start With Hard Numbers (Budgets You Enforce)

You need *explicit* token budgets. Assume your model has:

- `MODEL_CONTEXT_TOKENS` (example: 16,000)

You must reserve tokens for:

1. **Model output (the answer)**  
   - `RESERVED_OUTPUT_TOKENS` (example: 1,000–2,000)

2. **Prompt overhead** (system prompt + formatting + tool descriptions)  
   - `RESERVED_OVERHEAD_TOKENS` (example: 500–1,500)

Then your maximum safe input is:

- `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example:

- `MODEL_CONTEXT_TOKENS = 16000`
- `RESERVED_OUTPUT_TOKENS = 1500`
- `RESERVED_OVERHEAD_TOKENS = 800`
- `MAX_INPUT_TOKENS = 13700`

Now define chunk budgets:

- `MAX_CHUNK_TOKENS` (input per summarization call): 2,500–4,000  
  Example: `3000`
- `TARGET_CHUNK_SUMMARY_TOKENS`: 250–500  
  Example: `350`
- `TARGET_GROUP_SUMMARY_TOKENS`: 300–600  
  Example: `450`
- `TARGET_GLOBAL_SUMMARY_TOKENS`: 600–1,200  
  Example: `900`
- For ongoing chat:
  - `MEMORY_TOKEN_LIMIT`: 400–800 (example: `600`)
  - `RECENT_WINDOW_TOKEN_LIMIT`: 2,000–4,000 (example: `3000`)

### Token counting is mandatory
Implement `token_count(text)` using the model’s tokenizer (e.g., `tiktoken` for OpenAI models). Don’t guess by characters.

---

## 2) Store the Conversation in a Structured Format

Represent each message as a record:

```json
[
  {"id": 1, "role": "user", "content": "...", "ts": "..."},
  {"id": 2, "role": "assistant", "content": "...", "ts": "..."}
]
```

Why IDs matter:

- You can track exactly which messages are covered by which chunk summary.
- You can later re-summarize only part of the history if needed.
- You can do retrieval (“give me the chunk summary that covers message 120”).

---

## 3) Step A — Chunk the Conversation (Token-Aware, Boundary-Safe)

### Goal
Split the message list into chunks where each chunk’s message text fits under `MAX_CHUNK_TOKENS`.

### Pseudocode (exact logic)

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current = []
    current_tokens = 0

    for msg in messages:
        text = msg.role + ": " + msg.content
        t = token_count(text)

        if t > MAX_CHUNK_TOKENS:
            # Edge case: one message is bigger than allowed.
            # Strategy: summarize that single message separately, then treat that summary as a "replacement message".
            one_summary = summarize_single_message(msg)
            replacement = {role: "assistant", content: "(summary of oversized message) " + one_summary}
            # push current chunk if it has content
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

### Important edge cases you must handle
- **Oversized single message** (user pasted a huge log): summarize it alone first.
- **Never split inside a message** unless you have a special “split by paragraphs then summarize” routine.

---

## 4) Step B — Summarize Each Chunk With a Strict Schema

### Why strict schema matters
If you just say “summarize,” the model may produce a narrative that loses:
- constraints
- key decisions
- TODOs
- important numbers/paths/commands

You want a summary that can actually substitute for raw text later.

### Chunk summarization prompt (copy-paste)

Use this *for every chunk*:

> You are summarizing a segment of a long user–assistant conversation.  
>  
> PURPOSE: Create a compact, information-dense summary that can replace the raw messages in future prompts.  
>  
> MUST INCLUDE (only if present and important):  
> - User goals/questions/tasks introduced or progressed in this segment  
> - Key facts and constraints (numbers, deadlines, environment, tool versions, file paths, commands)  
> - Assistant’s substantive contributions (plans, reasoning, proposed architecture, fixes) described concisely  
> - Decisions/outcomes reached (what was chosen, what was rejected)  
> - Open questions / TODOs created or left unresolved  
>  
> MUST EXCLUDE OR MINIMIZE:  
> - Greetings, filler, politeness, repetition  
> - Detail that does not affect decisions or future work  
>  
> OUTPUT FORMAT (plain text, no JSON, no code fences):  
> - Topics:  
> - User Goals:  
> - Key Facts / Constraints:  
> - Assistant Actions / Suggestions:  
> - Decisions / Outcomes:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 350 tokens. Be extremely specific and concise. Preserve all important numbers and commands.  
>  
> [BEGIN SEGMENT]  
> <paste messages from this chunk with role labels>  
> [END SEGMENT]

### Store the result with metadata
For each chunk summary, store:

```json
{
  "chunk_id": 7,
  "message_ids": [120,121,122],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

Now you have **first-level summaries**.

---

## 5) Step C — Summarize the Summaries (Hierarchical Compression)

If you have many chunk summaries, they may still exceed your global budget.

### 5.1 Grouping
Choose group size so the input fits:

- If chunk summaries are ~350 tokens, `group_size = 8–10` is typical.

Pseudocode:

```pseudo
function group_items(items, group_size):
    groups = []
    for i in range(0, len(items), group_size):
        groups.append(items[i : i + group_size])
    return groups
```

### 5.2 “Summaries of summaries” prompt (copy-paste)

> You are compressing multiple chunk summaries of a conversation into one higher-level summary.  
>  
> GOAL: Merge redundancy while preserving durable, decision-relevant information:  
> - Stable user profile/preferences/constraints  
> - Major solutions/architectures and why they were chosen  
> - Key decisions/outcomes  
> - Long-running threads and their status  
> - Remaining open questions/TODOs  
>  
> OUTPUT FORMAT:  
> - Themes:  
> - Timeline / Phases (1–N):  
> - Persistent Facts / Preferences:  
> - Key Decisions / Outcomes:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 450 tokens. Remove repetition aggressively.  
>  
> [BEGIN INPUT SUMMARIES]  
> <paste 8–10 chunk summaries in chronological order>  
> [END INPUT SUMMARIES]

Run this for each group → you get **second-level summaries**.

### 5.3 Repeat if necessary
If second-level summaries still don’t fit:

- group them again
- summarize again

Until you have either:
- one global summary, or
- a small set of global summaries that together fit your target.

This is a summarization “tree”:
- raw messages → chunk summaries → group summaries → global summary

---

## 6) Step D — Produce a Bounded Long-Term Memory Object

Now take the global summary and convert it into a **memory** you can inject into future prompts.

### Memory creation prompt (copy-paste)

> Convert the following conversation summary into a long-term memory for future turns.  
>  
> Keep only information likely to matter later:  
> - User profile (skill level, preferences, constraints)  
> - Environment constraints (OS, tooling, languages, repos)  
> - Ongoing projects and current status  
> - Key decisions and short rationale  
> - Open questions / TODOs  
>  
> Output format:  
> - User Profile:  
> - Preferences:  
> - Constraints / Environment:  
> - Projects / Status:  
> - Key Decisions / Rationale:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 600 tokens. If you must drop details, drop the least durable ones first.  
>  
> [BEGIN SUMMARY]  
> <global summary text>  
> [END SUMMARY]

Store this as `long_term_memory`.

---

## 7) Live / Ongoing Chat: Rolling Memory Updates (So You Don’t Re-Summarize Everything)

In a live system, you keep two things:

1. `long_term_memory` (bounded, e.g. ≤ 600 tokens)
2. `recent_messages` (raw, e.g. last 3,000 tokens)

### 7.1 Update trigger (explicit rule)
Whenever:

`token_count(system + long_term_memory + recent_messages + current_user_msg) > MAX_INPUT_TOKENS * 0.9`

…you compress older recent messages into memory.

### 7.2 Select what to compress
Keep the last K turns raw (e.g., last 8 messages). Summarize the rest (or the oldest part of it).

```pseudo
function select_old_segment(recent_messages, keep_last=8, max_tokens=2000):
    if len(recent_messages) <= keep_last:
        return []

    candidates = recent_messages[0 : -keep_last]
    return trim_to_token_limit(candidates, max_tokens)
```

### 7.3 Summarize that old segment
Use the same chunk summarization prompt to create a `segment_summary`.

### 7.4 Merge into memory (update-memory prompt)
Use a dedicated prompt to merge and also keep memory small:

> You maintain a bounded long-term memory of a conversation.  
>  
> CURRENT MEMORY:  
> <existing memory>  
>  
> NEW INFORMATION (summary of older recent turns):  
> <segment summary>  
>  
> Update the memory by:  
> - Adding important persistent info  
> - Updating/overwriting info that changed  
> - Removing low-value/obsolete details to stay under the size limit  
>  
> OUTPUT the UPDATED MEMORY ONLY in this format:  
> - User Profile:  
> - Preferences:  
> - Constraints / Environment:  
> - Projects / Status:  
> - Key Decisions / Rationale:  
> - Open Questions / TODOs:  
>  
> HARD LENGTH LIMIT: ≤ 600 tokens.

Then:
- replace `long_term_memory` with the updated memory
- delete the summarized raw messages from `recent_messages`

Now you’re back under budget.

---

## 8) Retrieval (Optional but Often Best)

If you want higher accuracy than a single “memory blob”:

1. Store **chunk summaries** and compute embeddings for each.
2. When a new user question arrives:
   - embed the question
   - retrieve top-K most relevant chunk summaries
3. Build prompt with:
   - system message
   - long_term_memory
   - top-K retrieved summaries (small)
   - recent_messages

This gives “selective recall” without expanding context endlessly.

---

## 9) Prompt Assembly Pattern (What You Actually Send When Answering)

A robust final prompt for answering looks like:

1. System instructions
2. Long-term memory (≤ 600 tokens)
3. Retrieved relevant past summaries (≤ 1–2k tokens total)
4. Recent raw conversation window (≤ 2–4k tokens)
5. Current user request

This scales indefinitely because every component has a hard cap.

---

## 10) Common Failure Modes (And Fixes)

1. **Summary loses important numbers/commands**  
   Fix: explicitly instruct “preserve all numbers, file paths, commands, API names.”

2. **Memory grows without bound**  
   Fix: enforce strict memory token limit and require dropping low-value details.

3. **Summaries become too vague**  
   Fix: demand a schema with “Key Facts/Constraints” and “Decisions” and “TODOs”.

4. **You overflow anyway**  
   Fix: trigger summarization earlier (at 60–70% usage), not at 95–100%.

---

If you tell me:
- which model you’re using and its context size, and
- whether you need offline summarization or live rolling memory,
I can recommend exact budget numbers (chunk size, summary size, trigger thresholds) that typically work well for that setup.
