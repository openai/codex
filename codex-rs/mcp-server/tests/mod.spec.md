## Overview
`mcp-server/tests` aggregates integration coverage for the MCP server binary. Tests compile through `tests/all.rs` and live under `tests/suite`.

## Detailed Behavior
- `all.rs` declares the suite module, which currently focuses on Codex tool interactions in `suite/codex_tool.rs`.

## Broader Context
- Complements the Phase 2 MCP server specs by validating tool orchestration and approval flows end to end.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./all.rs.spec.md
  - ./suite/mod.rs.spec.md
  - ./suite/codex_tool.rs.spec.md
