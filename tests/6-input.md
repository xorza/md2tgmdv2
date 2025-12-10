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
