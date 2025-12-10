Below is a concrete recipe, written so you can implement it.

---

## 1. What You’re Trying To Achieve

Given:

- A long conversation: a list like  
  `messages = [{role: "user"|"assistant", content: string}, …]`
- An LLM with a fixed context limit (token budget), e.g. 8k / 16k / 32k / 128k tokens.
