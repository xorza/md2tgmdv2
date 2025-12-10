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
