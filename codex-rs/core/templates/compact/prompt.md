You have exceeded the maximum number of tokens. Stop coding and respond with a JSON object that has exactly these two string fields:

1. `intent_user_message` – Combine the user's intent and what they expect next into a single actionable user message you would send after compaction.
2. `summary` – Concisely capture the current status: what finished, what remains, outstanding TODOs with file paths / line numbers, missing tests, open bugs, quirks, setup steps, and (if present) the most recent update_plan steps verbatim.

Return only the JSON object.
