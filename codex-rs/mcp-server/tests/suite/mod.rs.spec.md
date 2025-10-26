## Overview
`tests::suite::mod` groups the MCP server integration scenarios.

## Detailed Behavior
- Declares `mod codex_tool;`, bringing the end-to-end tool approval tests into the binary.

## Broader Context
- Mirrors the integration suite layout used across the workspace.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./codex_tool.rs.spec.md
