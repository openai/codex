pub(crate) const THREAT_MODEL_SYSTEM_PROMPT: &str = r#"You are a senior application security engineer preparing a threat model.
Use the provided architecture specification and repository summary to enumerate realistic threats, prioritised by risk.
Prefer concrete, system-specific threats over generic checklists. When details are missing, state assumptions explicitly."#;

pub(crate) const THREAT_MODEL_PROMPT_TEMPLATE: &str = r#"# Repository Summary
{repository_summary}

# Architecture Specification
{combined_spec}

# In-Scope Locations
{locations}

# Task
Construct a concise threat model for the system. Focus on meaningful attacker goals and concrete impacts.

Some architecture details may already be present in the specification above. Do not restate large portions of the spec; instead, summarize and (when helpful) refer to the relevant spec section headings.

## Output Requirements
- Start with a short paragraph summarising the most important threat themes and high-risk areas.
- Include the following sections (keep them short and derived from the spec; 4–8 bullets each where applicable):
  - `## Primary components` (e.g., API gateway/auth/rate limiting, native preprocessing library, model runner/sandboxing, logging & metrics)
  - `## Trust boundaries` (use arrow notation such as `Internet → API Gateway`, `Gateway → Native library`, etc.)
  - `## Components & Trust Boundary Diagram` containing exactly one `mermaid flowchart TD` (or `flowchart LR`) diagram that shows the primary components and highlights trust boundaries (for example, with `subgraph` zones or annotations). Keep it compact (no more than ~12 nodes), label edges with the action/payload, and include a `title <descriptive label>` line inside the mermaid block.
  - `## Assets` as a 2-column table: `Asset` | `Why it matters`
  - `## Attacker model` with two sublists: `Capabilities` and `Non-capabilities`. Prefer realistic remote attacker assumptions; unless the spec indicates a shared-host or insider threat context, treat direct host filesystem tampering outside the application's own write surfaces as a non-capability.
  - `## Entry points` (endpoints, upload surfaces, parsing/decoding paths, error handling/logging)
- After those sections, add a short bullet list titled `## Top abuse paths` (3–8 items) before the threat table.
- Follow with a markdown table named `Threat Model` with columns: `Threat ID`, `Threat source`, `Prerequisites`, `Threat action`, `Threat impact`, `Impacted assets`, `Priority`, `Recommended mitigations`.
- Use integer IDs starting at 1. Priority must be one of `high`, `medium`, `low`.
- Keep prerequisite and mitigation text succinct (single sentence each).
"#;
