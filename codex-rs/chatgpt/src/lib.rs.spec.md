## Overview
`codex_chatgpt::lib` wires the crate’s modules together, exposing the CLI entrypoints and internal helpers that implement hosted task retrieval and diff application.

## Detailed Behavior
- Publicly re-exports:
  - `apply_command` module for applying hosted task diffs.
  - `get_task` module for fetching task metadata.
- Keeps `chatgpt_client` and `chatgpt_token` internal, ensuring callers depend on high-level APIs rather than manipulating HTTP/token state directly.

## Broader Context
- Serves as the binding point for the Codex CLI; other crates depend on this module to access the apply-command flow documented in the Phase 4 specs.

## Technical Debt
- None; file intentionally stays minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./apply_command.rs.spec.md
  - ./get_task.rs.spec.md
