pub(crate) const BUG_DEDUP_SYSTEM_PROMPT: &str = "You are a senior application security engineer clustering security review findings. Only respond with JSON Lines.";

pub(crate) const BUG_DEDUP_PROMPT_TEMPLATE_PASS1: &str = r#"
You will receive JSON objects describing security findings. Some are duplicates of the same underlying issue or multiple instances of the same root-cause vulnerability.

This is pass 1 (coarse). Use ONLY `title`, `severity`, and `sink_locations` to decide duplicates. Be conservative: if the titles and sink locations are not clearly the same issue, keep them separate.

Task: For EACH input finding, output exactly one JSON line in this exact schema:
{"id": <number>, "canonical_id": <number>, "reason": "<=20 words>"}

Rules:
- `id` must match an input `id`.
- `canonical_id` must be one of the input ids. Prefer the smallest id in the duplicate cluster.
- If the finding is unique, set `canonical_id` = `id`.
- Prefer merging multiple instances when they clearly share the same sink location(s) and would be fixed by the same patch/refactor/helper.
- If unsure, keep them separate.

Findings (one JSON object per line):
{findings}
"#;

pub(crate) const BUG_DEDUP_PROMPT_TEMPLATE_PASS2: &str = r#"
You will receive JSON objects describing security findings. Some are duplicates of the same underlying issue or multiple instances of the same root-cause vulnerability.

This is pass 2 (contextual). Use ONLY `description`, `impact`, and `root_cause` to decide duplicates.

Task: For EACH input finding, output exactly one JSON line in this exact schema:
{"id": <number>, "canonical_id": <number>, "reason": "<=20 words>"}

Rules:
- `id` must match an input `id`.
- `canonical_id` must be one of the input ids. Prefer the smallest id in the duplicate cluster.
- If the finding is unique, set `canonical_id` = `id`.
- Prefer merging multiple instances when they clearly share the same root cause and would be fixed by the same patch/refactor/helper, even across different files/lines.
- Use the description and impact to avoid over-broad merges: do NOT merge just because both mention a broad category (e.g., “buffer overflow”) if the root causes/fixes differ (e.g., stack `strcpy/strcat` path building vs integer truncation in allocation sizing).

Findings (one JSON object per line):
{findings}
"#;
