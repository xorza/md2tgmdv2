To summarize a conversation with an LLM when the full transcript does **not** fit in the token budget, you need a **deterministic compression pipeline**. The core trick is: **summarize in pieces, then summarize the summaries**, and maintain a **bounded “memory”** you can carry forward.

Below is a very specific, verbose, implementable approach.

---

## 1) Decide your hard limits (don’t guess)

You need these numbers before you write code:

1. **Model context size**  
   - `MODEL_CONTEXT_TOKENS` (e.g., 8k / 16k / 32k / 128k).

2. **Reserve room for the model’s answer**  
   You can’t use the entire context for input. Pick:
   - `RESERVED_OUTPUT_TOKENS` (e.g., 800–2000 depending on how long answers you want)

3. **Reserve room for “instructions + scaffolding”**  
   System prompt + policies + formatting:
   - `RESERVED_OVERHEAD_TOKENS` (e.g., 300–1200)

4. Compute your **maximum safe input budget**:
   - `MAX_INPUT_TOKENS = MODEL_CONTEXT_TOKENS - RESERVED_OUTPUT_TOKENS - RESERVED_OVERHEAD_TOKENS`

Example (16k model):
- `MODEL_CONTEXT_TOKENS = 16000`
- `RESERVED_OUTPUT_TOKENS = 1500`
- `RESERVED_OVERHEAD_TOKENS = 800`
- `MAX_INPUT_TOKENS = 13700`

Now pick chunk sizes:
- `MAX_CHUNK_TOKENS = 2500–4000` (for each summarization call)
- `TARGET_CHUNK_SUMMARY_TOKENS = 250–500`
- `TARGET_GLOBAL_SUMMARY_TOKENS = 600–1200`
- `MEMORY_TOKEN_LIMIT = 400–800` (for ongoing chat memory)

**Important:** implement `token_count(text)` using the model tokenizer (for OpenAI models, that typically means using a tokenizer library like `tiktoken`). Do not approximate by character count.

---

## 2) Represent the conversation in a machine-friendly form

Store messages as:

```json
[
  {"id": 1, "role": "user", "content": "...", "timestamp": "..."},
  {"id": 2, "role": "assistant", "content": "...", "timestamp": "..."},
  ...
]
```

Keep stable IDs. This lets you:
- cite what chunk contains what,
- update summaries later,
- retrieve relevant pieces.

---

## 3) Step 1: Chunk the conversation (token-aware, message boundary safe)

You must chunk by message boundaries, not arbitrary splits.

### Chunking algorithm (precise logic)
- Walk messages from oldest to newest
- Add message to current chunk until adding it would exceed `MAX_CHUNK_TOKENS`
- Close the chunk, start a new one

Pseudocode:

```pseudo
function chunk_messages(messages, MAX_CHUNK_TOKENS):
    chunks = []
    current = []
    current_tokens = 0

    for msg in messages:
        msg_text = msg.role + ": " + msg.content
        t = token_count(msg_text)

        # Edge case: single message too long
        if t > MAX_CHUNK_TOKENS:
            # Option A: summarize this message alone (recommended)
            chunks.append([msg])  # but mark it oversized and handle separately
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

### Oversized single message handling (important)
If a single message is too large:
- Run a special “summarize this one message” call
- Replace it with its short summary for downstream chunking

This prevents pipeline failure on huge pasted logs.

---

## 4) Step 2: Summarize each chunk with a strict schema

Do **not** ask the model “summarize this” vaguely. You want a summary that is useful later, not a nice narrative.

### Use a structured template (recommended)

**Chunk summary prompt (copy/paste)**

> You are summarizing a segment of a long user–assistant conversation.  
>  
> PURPOSE: Create a compact summary that can replace the raw messages in future prompts.  
>  
> MUST CAPTURE:
> 1) User goals/questions asked in this segment  
> 2) Concrete facts and constraints stated (numbers, deadlines, environment, preferences)  
> 3) Assistant’s substantive outputs (plans, explanations, decisions, code approaches—describe, don’t paste long code)  
> 4) Decisions/outcomes reached  
> 5) Open issues / TODOs created or left unresolved  
>  
> MUST AVOID:
> - greetings, filler, repeated statements  
> - non-persistent details unless they change decisions  
>  
> OUTPUT FORMAT (plain text, no JSON, no code fences):
> - Topics:
> - User Goals:
> - Key Facts / Constraints:
> - Assistant Actions / Suggestions:
> - Decisions / Outcomes:
> - Open Questions / TODOs:
>  
> HARD LIMIT: <= 350 tokens. Be information-dense.  
>  
> [BEGIN SEGMENT]
> <paste chunk messages here with role labels>
> [END SEGMENT]

For each chunk, you call the LLM and store:

```json
{
  "chunk_id": 7,
  "message_ids": [120, 121, 122, ...],
  "summary": "Topics: ...\nUser Goals: ...\n..."
}
```

This is your **first-level summary**.

---

## 5) Step 3: Summarize the summaries (hierarchical compression)

If you have many chunk summaries, even they might not fit.

So you compress again:

1. Group chunk summaries into “summary groups” that fit into `MAX_CHUNK_TOKENS`
2. Summarize each group into a second-level summary
3. Repeat until you have 1 global summary

### Grouping logic
If each chunk summary is ~350 tokens, then a group size of ~8–10 often fits (plus prompt overhead).

Pseudocode:

```pseudo
function group_summaries(chunk_summaries, group_size):
    return [chunk_summaries[i:i+group_size] for i in range(0, n, group_size)]
```

### “Summaries of summaries” prompt

> You are compressing multiple summaries of a conversation into a higher-level summary.  
>  
> GOAL: Merge redundancies and preserve only durable, decision-relevant information:
> - stable user preferences/constraints
> - major solutions and conclusions
> - important technical decisions/architectures
> - unresolved issues / TODOs
>  
> OUTPUT FORMAT:
> - Themes:
> - Timeline (major phases):
> - Persistent Facts / Preferences:
> - Key Decisions:
> - Open TODOs:
>  
> HARD LIMIT: <= 450 tokens.
>  
> [BEGIN INPUT SUMMARIES]
> <paste 8–10 chunk summaries in chronological order>
> [END INPUT SUMMARIES]

Repeat until total is within your global target size.

---

## 6) Step 4: Create a “Long-Term Memory” object (bounded, reusable)

Once you have a global summary (or a small number of them), convert it into a compact memory you can inject into future prompts.

### Memory-building prompt

> Convert the following conversation summary into a long-term memory for future turns.  
>  
> Keep only information likely to matter later:
> - user profile (skill level, preferences)
> - constraints (tools, deadlines, environment)
> - ongoing projects and status
> - key decisions and rationale (brief)
> - open questions / next steps
>  
> Format:
> - User Profile:
> - Preferences:
> - Constraints:
> - Projects / Status:
> - Key Decisions:
> - Open Questions / TODOs:
>  
> HARD LIMIT: <= 600 tokens.
>  
> [BEGIN SUMMARY]
> <global summary here>
> [END SUMMARY]

Store this as `long_term_memory`.

---

## 7) Ongoing conversation: Maintain rolling memory (so you don’t re-summarize everything)

If this is a live chat system, you want:

- `long_term_memory` (<= 600 tokens)
- `recent_messages` (raw, last N tokens)
- optionally `chunk_summaries` for retrieval

### Update policy (very specific)
Every time a new message arrives:

1. Append to `recent_messages`
2. If `token_count(long_term_memory + recent_messages + system)` exceeds your budget:
   - select the oldest part of `recent_messages` (e.g., everything except last 6–10 turns)
   - summarize that segment
   - merge it into `long_term_memory` via an “update memory” call
   - drop the summarized raw messages

#### Update-memory prompt (copy/paste)

> You maintain a bounded long-term memory of a user–assistant conversation.  
>  
> CURRENT MEMORY:
> <existing memory>
>  
> NEW INFORMATION (summary of older recent turns):
> <segment summary>
>  
> Update the memory:
> - Add important persistent info
> - Update/overwrite anything that changed
> - Remove low-value or obsolete details to stay under size limit
>  
> Output the UPDATED MEMORY ONLY in this format:
> - User Profile:
> - Preferences:
> - Constraints:
> - Projects / Status:
> - Key Decisions:
> - Open Questions / TODOs:
>  
> HARD LIMIT: <= 600 tokens.

This keeps you always within budget.

---

## 8) Retrieval (optional but highly effective)

Instead of only having one memory blob, store:
- chunk summaries + embeddings
- group summaries + embeddings

At query time:
1. Embed the user’s new question
2. Retrieve the top-K relevant summaries
3. Put only those in the prompt (plus long_term_memory + recent messages)

This gives better accuracy than a single global summary, without blowing context.

---

## 9) The “final prompt assembly” pattern (what you actually send to answer)

When responding to a new user message, build input like:

1. **System instructions**
2. **Long-term memory** (bounded)
3. **Retrieved relevant summaries** (top-K)
4. **Recent raw messages** (short window)
5. **Current user request**

This pattern scales indefinitely because you always cap each component.

---

## 10) Common failure modes (and how to prevent them)

1. **Summaries become too vague**
   - Fix: enforce a schema and require “Key Facts / Constraints” + “Decisions” + “TODOs”.

2. **Important numbers/details lost**
   - Fix: explicit instruction: “preserve all numbers, commands, file paths, API names”.

3. **Memory grows over time**
   - Fix: strict token limit + “drop low-value details” requirement in update-memory prompt.

4. **You summarize too late and overflow**
   - Fix: start compressing at ~60–70% of budget, not at 95–100%.

5. **Model hallucinates missing past context**
   - Fix: in system prompt say: “If not in memory/retrieved context, ask the user or request logs.”

---

If you tell me:
- which model/context size you’re using,
- whether this is offline summarization or live chat,
- and your language (Python/JS),
I can give you a concrete implementation skeleton (including token counting and the exact “when to summarize” thresholds).
