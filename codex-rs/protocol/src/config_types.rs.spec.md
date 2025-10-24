## Overview
`protocol::config_types` enumerates configuration knobs that Codex exposes for model reasoning, verbosity, sandbox modes, and enforced login paths. The enums mirror OpenAI Responses API options so they can travel across the wire unchanged.

## Detailed Behavior
- `ReasoningEffort`, `ReasoningSummary`, and `Verbosity` map directly to the corresponding Responses API parameters, deriving `Display`, `Serialize`, `Deserialize`, `JsonSchema`, and `TS` with lowercase serialization to match API expectations.
- `SandboxMode` represents the high-level sandbox posture (`read-only`, `workspace-write`, `danger-full-access`) using kebab-case serialization to align with CLI flags and configuration files.
- `ForcedLoginMethod` distinguishes whether a user must authenticate via ChatGPT or API key flows, ensuring client UIs present the correct login path.
- All enums implement `EnumIter` or similar traits where helpful for UI iteration; default variants align with product defaults (e.g., `ReasoningEffort::Medium`).

## Broader Context
- These enums are embedded in the `Config` struct defined in `codex-core` and surfaced in configuration summaries. Any new API options must be added here to remain serializable across clients.
- Keeping serialization formats aligned with the external API prevents accidental breaking changes when forwarding requests to OpenAI.
- Context can't yet be determined for potential future sandbox levels or reasoning modes beyond the current set; additions should preserve backward compatibility through tagged enum variants.

## Technical Debt
- None observed; the enums faithfully reflect current API capabilities.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
