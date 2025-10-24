## Overview
`common::config_summary` converts a fully-resolved `codex_core::config::Config` into a vector of key/value pairs that user interfaces can render when previewing the active configuration. The module exports a single helper, `create_config_summary_entries`, which gathers commonly requested fields and encodes them into human-readable strings.

## Detailed Behavior
- Always includes entries for `workdir`, `model`, `provider`, `approval`, and `sandbox`, drawing directly from the `Config` structure. The sandbox description delegates to `summarize_sandbox_policy` to ensure consistent wording with other UI surfaces.
- When the active model provider uses the `Responses` wire API and the model family supports reasoning summaries, the function appends two additional fields:
  - `reasoning effort`: textual representation of the optional `model_reasoning_effort`, defaulting to `"none"` when unset.
  - `reasoning summaries`: string form of `model_reasoning_summary`, preserving existing formatting rules on the enum.
- Returns the entries in the order they were pushed so downstream renderers can rely on stable presentation without additional sorting.

## Broader Context
- The helper provides a shared source of truth for configuration displays consumed by the CLI, TUI, and MCP server dashboards. Any new fields added to `Config` that should surface to users must be wired through this function to maintain parity.
- Because it depends on `codex_core::config::Config`, changes to configuration semantics in `codex-core` can affect the summary without requiring modifications here, but the consuming specs should mention those invariants.
- Context can't yet be determined for how enterprise-only configuration fields (if any) should be summarized; revisit once those modules have corresponding specs.

## Technical Debt
- None observed within this module; logic is straightforward and matches configuration invariants described in `codex-core`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./sandbox_summary.rs.spec.md
