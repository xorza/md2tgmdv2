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

Code‑ish:

```pseudo
chunk_summaries = []

for i, chunk in enumerate(chunks):
    prompt = build_chunk_summary_prompt(chunk)  # uses text above
    summary_text = call_llm(prompt)
    chunk_summaries.append({
        "chunk_index": i,
        "summary": summary_text
    })
```

Now the entire long conversation is represented by, say, 50–200 chunk summaries.

---

### 3.3. Step 3 – Summarize the Summaries (Hierarchy)

If all `chunk_summaries` together are still too big for your use case, compress them again.

#### 3.3.1. Group summaries

Choose `group_size` so that `group_size * TARGET_CHUNK_SUMMARY_TOKENS` fits comfortably under `MAX_CHUNK_TOKENS`.

Example: if chunk summaries ≈300 tokens, `group_size = 10` ⇒ ~3000 tokens of input.

```pseudo
group_size = 10
groups = []
for i in range(0, len(chunk_summaries), group_size):
    groups.append(chunk_summaries[i : i + group_size])
```

Each `group` will be converted into a higher‑level summary.

#### 3.3.2. Prompt to compress a group of summaries:

> You are compressing multiple conversation summaries into a more compact higher‑level summary.  
>  
> **INPUT:**  
> A list of summaries, in chronological order, each summarizing a segment of a long user–assistant conversation.  
>  
> **GOAL:**  
> - Merge overlapping and redundant information.  
> - Preserve important, persistent information about:  
>   - user goals, preferences, constraints, and profile  
>   - key facts and data  
>   - main solution ideas, designs, or strategies  
>   - important decisions and outcomes  
>   - long‑running tasks or projects and their progress  
>   - important unresolved questions or TODOs  
> - Provide a sense of how the conversation evolved across these segments.  
>  
> **OUTPUT FORMAT (plain text, no JSON):**  
> - High‑Level Themes: bullet list of dominant themes/topics across all segments.  
> - Timeline / Phases: numbered list where each item briefly describes a phase (what the user wanted, what the assistant did, key changes or progress).  
> - Persistent User Info: bullet list of stable user profile info or preferences seen here.  
> - Key Decisions / Facts: bullet list of core facts, designs, or decisions that remain important.  
> - Open Issues / TODOs: bullet list of unresolved questions or tasks left after these segments.  
>  
> **LENGTH LIMIT:**  
> - Maximum 400 tokens. Aggressively remove repetition and low‑value detail.  
>  
> Now compress the following summaries into one higher‑level summary:  
>  
> [BEGIN SUMMARIES]  
> <insert the chunk summaries here, separated and in order>  
> [END SUMMARIES]

Pseudocode:

```pseudo
group_summaries = []

for group in groups:
    prompt = build_multi_summary_prompt(group)
    summary_text = call_llm(prompt)
    group_summaries.append(summary_text)
```

If the list `group_summaries` is still too long in total:

- Treat `group_summaries` as the new “chunk_summaries”.
- Repeat: group them, compress again.
- Continue until you can fit everything into **one global summary** of ≤ `TARGET_GLOBAL_SUMMARY_TOKENS`.

---

### 3.4. Step 4 – Convert Global Summary into a Reusable “Memory”

Now you have a global textual summary. Turn it into a small, structured memory object.

#### Prompt: global summary → long‑term memory

> You are constructing a compact long‑term memory from a global summary of a user–assistant conversation.  
>  
> **INPUT:**  
> A high‑level summary of the entire conversation.  
>  
> **TASK:**  
> Convert this into a structured memory that contains only information likely to be useful in future conversation turns.  
>  
> Focus on:  
> - Who the user is (skills, preferences, constraints, relevant background)  
> - Main projects, tasks, or topics and their status  
> - Important decisions, conclusions, designs, or facts that should not be forgotten  
> - Unresolved questions or TODOs that might be revisited later  
>  
> Ignore:  
> - Transient details, minor side comments, throwaway examples, and small talk  
>  
> **OUTPUT FORMAT (plain text, no JSON):**  
> - User Profile: bullet list of what we know about the user (skills, preferences, constraints, etc.).  
> - Projects / Topics: bullet list of the main projects or topics and their current status.  
> - Key Decisions / Facts: bullet list of core facts, designs, or conclusions that matter later.  
> - Open Questions / TODOs: bullet list of unresolved issues or planned next steps.  
>  
> **LENGTH LIMIT:**  
> - Maximum 600 tokens. Be very selective and concise.  
>  
> Now produce the long‑term memory from the following summary:  
>  
> [BEGIN HIGH‑LEVEL SUMMARY]  
> <insert your final global summary>  
> [END HIGH‑LEVEL SUMMARY]

The model’s output is your `long_term_memory` string: a compressed representation of the entire conversation that you can re‑inject into future prompts.

---

## 4. Online: Keep Summarizing As You Chat

When conversation keeps growing, you need a **rolling** solution:

- Maintain:
  - a small `long_term_memory` (bounded text), and
  - a list of `recent_messages` (raw most recent turns).

### 4.1. State

```pseudo
state = {
    "long_term_memory": "",  # string, ≤ MEMORY_TOKEN_LIMIT
    "recent_messages": []    # list of {role, content}
}
```

### 4.2. On Every New User Message

Algorithm:

```pseudo
function handle_user_message(user_text):
    # 1. Add new user message
    state.recent_messages.append({role: "user", content: user_text})

    # 2. See how big the prompt would be if we include memory + all recent messages
    prompt = build_answer_prompt(
        long_term_memory = state.long_term_memory,
        recent_messages = state.recent_messages
    )
    used_tokens = token_count(prompt)

    # 3. If context is getting large, compress older part of recent_messages
    if used_tokens > MODEL_CONTEXT_TOKENS * 0.7:
        old_segment = select_old_segment(state.recent_messages)
        if old_segment not empty:
            segment_summary = summarize_segment(old_segment)  # same chunk-summary prompt, but on this segment
            # Remove old_segment from recent_messages
            state.recent_messages = state.recent_messages - old_segment
            # Merge new summary into long_term_memory
            state.long_term_memory = update_memory(
                current_memory = state.long_term_memory,
                new_summary = segment_summary
            )

    # 4. Build final prompt using updated memory + remaining recent messages
    prompt = build_answer_prompt(
        long_term_memory = state.long_term_memory,
        recent_messages = state.recent_messages
    )

    # 5. Ask LLM for the reply
    assistant_reply = call_llm(prompt)

    # 6. Store reply as part of recent_messages
    state.recent_messages.append({role: "assistant", content: assistant_reply})

    return assistant_reply
```

### 4.3. Selecting Which Old Messages to Summarize

Simple policy:

- Always keep the last `K` messages raw,
- Summarize everything older, up to a token limit.

```pseudo
function select_old_segment(recent_messages):
    keep_last_turns = 8  # keep last 8 turns uncompressed

    if length(recent_messages) <= keep_last_turns:
        return []

    candidates = recent_messages[0 : -keep_last_turns]

    # Now trim candidates to, say, 2000 tokens.
    old_segment = trim_messages_to_token_limit(candidates, limit = 2000)

    return old_segment
```

`summarize_segment(old_segment)` just reuses the chunk summarization prompt, but applied to `old_segment`.

---

### 4.4. Updating Memory Using the LLM

You merge `segment_summary` into existing `long_term_memory` with another LLM call.

#### Prompt: update memory

> You maintain a long‑term memory for a user–assistant conversation.  
>  
> CURRENT_MEMORY:  
> <insert current long_term_memory>  
>  
> NEW_INFORMATION (summary of a recent conversation segment):  
> <insert segment_summary>  
>  
> **TASK:**  
> Update CURRENT_MEMORY so that it:  
> - Adds any new, important long‑term information from NEW_INFORMATION.  
> - Updates or corrects information that has clearly changed.  
> - Optionally removes or shrinks details that are now obsolete or clearly unimportant.  
>  
> Focus on:  
> - Persistent user preferences, constraints, and profile info  
> - On‑going projects, tasks, or topics and their updated status  
> - Important decisions, designs, or conclusions that will matter later  
> - Open questions or TODOs that might be revisited  
>  
> Ignore or minimize:  
> - Transient details, one‑off examples, and minor small talk  
>  
> **OUTPUT:**  
> Return the UPDATED_MEMORY only, in this structure (plain text):  
> - User Profile  
> - Projects / Topics  
> - Key Decisions / Facts  
> - Open Questions / TODOs  
>  
> **LENGTH LIMIT:**  
> - Maximum 600 tokens. Compress aggressively and drop low‑importance details if necessary.

Implementation:

```pseudo
function update_memory(current_memory, new_summary):
    prompt = build_update_memory_prompt(current_memory, new_summary)
    updated = call_llm(prompt)
    return updated
```

This keeps `long_term_memory`:

- Under `MEMORY_TOKEN_LIMIT`, and
- Reflecting the whole conversation so far.

---

### 4.5. Building the Actual Prompt for Answers

To let the LLM answer the user using compressed history:

```pseudo
function build_answer_prompt(long_term_memory, recent_messages):
    text = ""
    text += "SYSTEM: You are an AI assistant. You have access to a summarized long-term memory of the user and our past conversation. Use it when relevant, but do not invent details that are not present in the memory or the recent messages.\n\n"

    if long_term_memory is not empty:
        text += "[LONG-TERM MEMORY]\n"
        text += long_term_memory + "\n\n"

    text += "[RECENT CONVERSATION]\n"
    for msg in recent_messages:
        text += msg.role + ": " + msg.content + "\n"

    return text
```

You send this (or an equivalent structured version) as context to the LLM, ensure `token_count(text)` + expected answer length ≤ `MODEL_CONTEXT_TOKENS`, and you’re good.

---

## 5. Core Rules to Remember

1. **Never send the full raw conversation once it’s large.**  
   Always send:
   - A compressed long‑term memory,
   - A recent window of raw messages.

2. **Use the LLM in several passes:**
   - Chunk‑level summarization,
   - Hierarchical compression (if needed),
   - Memory creation,
   - Memory updates.

3. **Prompts must be explicit and structured.**  
   Say exactly:
   - What to keep (goals, constraints, facts, decisions, open questions),
   - What to drop (small talk, repetition, incidental details),
   - How long the output can be.

4. **Always hard‑cap memory size.**  
   Re‑compress memory when it gets large by feeding it back into an “update/compress yourself” prompt.

5. **Summarize earlier, not at the last second.**  
   Start summarizing when context is around 60–70% full, leaving breathing room.

---

If you tell me:

- Which model you’re actually using (and its context length),
- What language you’re coding in,

I can translate this into near‑ready code with real functions and approximate numeric values filled in.
