pub(crate) const BUG_DEDUP_SYSTEM_PROMPT: &str = "You are a senior application security engineer deduplicating security review findings. Only respond with JSON Lines.";

pub(crate) const BUG_DEDUP_PROMPT_TEMPLATE: &str = r#"
You will receive JSON objects describing security findings. Some are duplicates of the same underlying root cause, repeated across files or phrased differently.

Task: For EACH input finding, output exactly one JSON line in this exact schema:
{"id": <number>, "canonical_id": <number>, "confidence": <0-1>, "reason": "<=12 words>"}

Rules:
- `id` must match an input `id`.
- `canonical_id` must be one of the input ids. Prefer the smallest id in the duplicate cluster.
- If the finding is unique, set `canonical_id` = `id`.
- Only mark a finding as a duplicate when you are highly confident (otherwise keep it unique).
- Merge findings that share the same root cause and would be fixed by the same patch, even if they appear in multiple files.
- Do NOT merge findings that are only in the same broad category (e.g., "buffer overflow") but differ in root cause or fix.

Findings (one JSON object per line):
{findings}
"#;
