You are performing a CONTEXT CHECKPOINT COMPACTION for a tool. You must output a single valid JSON object with the following schema:

{
"type": "object",
"properties": {
"intent_user_message": { "type": "string" },
"summary": { "type": "string" }
},
"required": ["intent_user_message", "summary"],
"additionalProperties": false
}

STRICT OUTPUT RULES
- Output a single valid JSON object only (no Markdown, no fences, no commentary).
- Must parse with serde_json::from_str.
- UTF-8 only.
- Up to ~4000 tokens total.

GOAL
Reconstruct the SINGLE ACTIVE TASK THREAD and all critical short-term context so the conversation can continue seamlessly after reset.

ACTIVE TASK SELECTION
- Identify the task thread with in-progress work (code edits, plans, tests, IDs, env state).
- Do not switch tasks due to incidental questions.
- If uncertainty exists, include the candidate context and mark unknowns.

OUTPUT FIELDS

"intent_user_message"
A consolidated user-side directive for re-injection after reset, containing:

1) Minimal machine-only context necessary to interpret the task
   (paths, APIs, configs, constraints, design notes, etc.)

2) The ORIGINAL user request that defined the active task, verbatim:
   <VERBATIM_REQUEST_START>
   ...original user text...
   <VERBATIM_REQUEST_END>

3) The most recent user messages relevant to continuing execution,
   **including clarifications, corrections, follow-ups, parameter overrides, and sub-tasks**.

Use this delimiter block for that (raw text, no quotes or escapes):
<RECENT_USER_CONTEXT_START>
...verbatim recent user messages (~2–6 turns or whatever is needed)...
<RECENT_USER_CONTEXT_END>

Rules for both blocks:
- EXACT text.
- Put the start/end tags on their own lines without indentation.

"summary"
A machine continuation context containing:

- current status/phase
- authoritative excerpts (files, functions, diffs, snippets)
- internal state and intermediate values
- pending steps and design notes
- environment context (paths, branches, configs, run IDs)
- test state or execution buffer if relevant
- explicit unknowns as "UNKNOWN"
- brief note on non-active threads only if helpful
- final cursor line:
  RESUME_AT: <next precise action>

SELF-CHECK BEFORE EMITTING (do not output)
- Valid JSON object only?
- intent_user_message includes both blocks:
  <VERBATIM_REQUEST_START> … <VERBATIM_REQUEST_END>
  <RECENT_USER_CONTEXT_START> … <RECENT_USER_CONTEXT_END>
- Enough granular state to continue without prior messages?
- summary ends with a concrete RESUME_AT?