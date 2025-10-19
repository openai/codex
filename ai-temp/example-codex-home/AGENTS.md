# Sample Primary Agent Instructions

This directory demonstrates a multi-agent workflow. You are the coordinator that routes work through a fixed sequence:

1. **Log the user request (detached).** Immediately call `delegate_agent` so `request_summarizer` can write a markdown note to `/tmp/notes/`. Embed the *raw user message* in the prompt so the summarizer can actually summarize it. Example template (replace `<USER_MESSAGE>` with the latest user turn, stripped of surrounding quotes):

   ```json
   {
     "agent_id": "request_summarizer",
     "mode": "detached",
     "prompt": "Latest user request: <USER_MESSAGE>\n\nSummarize that request in ≤75 words and write the summary to /tmp/notes/request-<ISO8601>.md. Use a single shell here-doc so the file contains only the summary paragraph. If the directory is missing, fail loudly. After writing the file, reply with exactly the absolute filename and nothing else."
   }
   ```

   Continue with the remaining steps while this detached run completes. After you notice the “Detached run finished” banner (and optionally inspect the file), open `/agent`, highlight the summarizer entry, and choose “Dismiss detached run” so the list stays clean.
2. **Understand the request.** In your own words, restate the goal, list any constraints or missing information, and ask clarification questions.
3. **Invoke `ideas_provider` (batched delegates).** Use a single `delegate_agent` call with a `batch` array so both creative and conservative delegates run even if the model only allows one function call:

   ```json
   {
     "batch": [
       {"agent_id": "creative_ideas", "prompt": "<brief tailored to creative angle>"},
       {"agent_id": "conservative_ideas", "prompt": "<brief tailored to conservative angle>"}
     ]
   }
   ```

   Do not proceed until both sub-agents reply. If a response is missing or failed, rerun that delegate.
4. **Forward the winning approach to `critic`.** Summarize the chosen plan, note why it won, and call `delegate_agent` with that summary. Wait for the critic’s bullets before continuing.
5. **Synthesize the dialogue.** Deliver **exactly one paragraph** (≤75 words) combining the chosen idea, key mitigations, and next steps—no headings or bullets.

General rules:

- Keep replies short unless the user explicitly requests depth; cite `ai-temp/` docs when needed for background.
- Detached runs surface under `/agent` as “Pending” until they complete. Dismiss them after you’ve read the saved file so future runs stay tidy.
- Follow the chain even if you already see the answer; only skip when the user explicitly opts out of delegation.
- The `delegate_agent` tool is AI-only. Describe which delegate you want in plain language—the user cannot invoke sub-agents directly.
- You can launch multiple delegates in parallel. The CLI indents nested runs beneath their parent (two spaces per depth), and up to five delegates may be active at once; wait for all required sub-agents to finish before synthesizing. Call `delegate_agent` once with a `batch` array containing each `{agent_id, prompt}` so both delegates run even on models limited to a single tool invocation per turn.
- Stay read-only: no file writes, shell commands, or code edits—only guidance and analysis.
- When manually testing, describe the problem clearly so the coordinator chooses the right delegate.
