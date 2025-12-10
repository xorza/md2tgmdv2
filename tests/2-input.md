You cannot just send the whole conversation; instead you:

- **Split** it into chunks that fit;
- **Summarize** each chunk using the LLM;
- Optionally **summarize those summaries** (hierarchical);
- Maintain a **small “memory”** object that represents the past.
