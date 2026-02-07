pub(crate) const BUG_DEDUP_SYSTEM_PROMPT: &str = "You are a senior application security engineer clustering security review findings. Only respond with JSON Lines.";

pub(crate) const BUG_DEDUP_PROMPT_TEMPLATE_PASS1: &str = r#"
You will receive JSON objects describing security findings. Some are duplicates of the same underlying issue or multiple instances of the same root-cause vulnerability.

This is pass 1 (coarse grouping gate). Use ONLY `title`, `severity`, and `sink_locations` to decide duplicates.

Goal: identify high-confidence duplicate candidates while avoiding false merges.

Task: For EACH input finding, output exactly one JSON line in this exact schema:
{"id": <number>, "canonical_id": <number>, "reason": "<short rationale>"}

Rules:
- `id` must match an input `id`.
- `canonical_id` must be one of the input ids. Prefer the smallest id in the duplicate cluster.
- If the finding is unique, set `canonical_id` = `id`.
- Merge only when ALL are true:
  1) Titles describe the same sink/operation failure pattern.
  2) `sink_locations` overlap or clearly normalize to the same function/endpoint/sink region.
  3) A single code change (same helper/check/refactor) would remediate both.
- Keep separate when ANY are true:
  - Same broad class but different sink locations or different fixes.
  - Same file/component but different vulnerable function/path.
  - One finding appears to be a consequence of another rather than the same defect.
- Favor under-merging when uncertain.
- Enforce transitive consistency across the whole set (if A->B and B->C, then A/B/C should share one canonical id).
- Keep `reason` concise and specific (mention the key sink/fix signal).

Findings (one JSON object per line):
{findings}
"#;

pub(crate) const BUG_DEDUP_PROMPT_TEMPLATE_PASS2: &str = r#"
You will receive JSON objects describing security findings. Some are duplicates of the same underlying issue or multiple instances of the same root-cause vulnerability.

This is pass 2 (contextual grouping). Use ONLY `description`, `impact`, and `root_cause` to decide duplicates.

Goal: consolidate true root-cause duplicates, including multi-file manifestations of the same bug, while avoiding over-broad merges.

Task: For EACH input finding, output exactly one JSON line in this exact schema:
{"id": <number>, "canonical_id": <number>, "reason": "<short rationale>"}

Rules:
- `id` must match an input `id`.
- `canonical_id` must be one of the input ids. Prefer the smallest id in the duplicate cluster.
- If the finding is unique, set `canonical_id` = `id`.
- Merge only when ALL are true:
  1) The same root-cause mechanism is described.
  2) The primary attacker primitive/abuse path is materially the same.
  3) A shared remediation strategy would fix all grouped findings.
- Prefer merging cross-file/endpoints when one missing guard/helper/invariant causes repeated manifestations.
- Keep separate when ANY are true:
  - Same vulnerability class label but different root causes or fixes.
  - Different trust boundaries, prerequisites, or attacker control assumptions.
  - Different primary impact chain (e.g., authz bypass vs memory corruption) even if both are high severity.
  - One finding is merely a downstream effect/reporting symptom of another.
- If ambiguous, keep them separate.
- Enforce transitive consistency across the full group.
- Keep `reason` concise and specific (mention root cause + fix signal).

Findings (one JSON object per line):
{findings}
"#;
