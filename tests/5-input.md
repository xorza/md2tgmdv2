
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
