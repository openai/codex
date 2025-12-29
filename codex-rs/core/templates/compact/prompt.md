You are performing a CONTEXT CHECKPOINT COMPACTION. Create a handoff summary so another LLM can continue the single active task (the most recent user request that is still in progress).

Rules:
- Focus only on the active task. Do not restate or reopen earlier, completed tasks.
- Do not propose audits or re-checking unless the user explicitly asked.
- If earlier tasks exist, note them as completed in one short bullet.

Use this format:
- Active task:
- Progress so far:
- Next steps (active task only):
- Key context / constraints / preferences (still relevant only):
- Completed tasks (optional, brief):

Be concise and structured so the next LLM can continue without broad backtracking.
