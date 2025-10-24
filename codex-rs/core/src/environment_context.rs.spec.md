## Overview
`core::environment_context` captures the execution settings for a Codex turn (cwd, approval policy, sandbox mode, network access, writable roots, shell) and serializes them into the tagged XML block sent to models. It also provides diffing helpers so Codex emits context updates only when something changes.

## Detailed Behavior
- `EnvironmentContext::new` maps from `SandboxPolicy` into `SandboxMode`, `NetworkAccess`, and writable roots, normalizing empty lists to `None` and deriving network access flags for workspace-write policies. Shell information is retained only when provided explicitly.
- `equals_except_shell` compares two contexts while ignoring the shell, allowing turn-by-turn equality checks even though shells are immutable after the first turn.
- `EnvironmentContext::diff` inspects two `TurnContext`s and records only the fields that changed, returning a sparse context suitable for incremental updates.
- `From<&TurnContext>` builds a full context snapshot for prompt injection at the start of a turn.
- `serialize_to_xml` renders the context into the `<environment_context>…</environment_context>` block using the protocol constants. Writable roots become nested `<root>` elements; missing fields are omitted entirely.
- `From<EnvironmentContext> for ResponseItem` wraps the serialized block into a user message so downstream components can embed environment metadata into the model input stream.

## Broader Context
- `codex.rs` emits environment context messages alongside user instructions to keep the model informed about cwd, policies, and available roots. Tool orchestration also references these fields when deciding sandbox strategy.
- Network flags align with the harness policies exposed in `SandboxPolicy`, ensuring responses match real command permissions. Client specs (`client.rs.spec.md`) describe how these tags reach the model.
- Context can't yet be determined for future fields (e.g., OS metadata); extending the struct will require updating both XML serialization and equality helpers.

## Technical Debt
- The XML serialization is manual and duplicated across related modules (`user_instructions`); extracting a shared formatter would reduce drift and improve testability.
- `serialize_to_xml` ignores errors from `PathBuf::to_string_lossy`; providing structured output (e.g., JSON payloads) would avoid lossy conversions for non-UTF8 paths.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Factor the XML serialization helpers into a shared utility so environment and instruction blocks stay in sync and gain better testing coverage.
    - Consider offering a structured (JSON) representation of writable roots to avoid lossy path conversions when non-UTF8 characters are present.
related_specs:
  - ../mod.spec.md
  - ./project_doc.rs.spec.md
  - ./user_instructions.rs.spec.md
