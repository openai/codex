## Overview
`protocol::plan_tool` defines the payloads exchanged with the Codex plan management tool (exposed via MCP). It supplies the schema for reporting plan status and updating step-by-step execution plans.

## Detailed Behavior
- `StepStatus` enumerates the allowed states (`Pending`, `InProgress`, `Completed`) for individual plan steps, matching the status vocabulary used by the VS Code TODO MCP tool.
- `PlanItemArg` captures a single plan entry with its descriptive `step` text and `status`. The struct denies unknown fields to catch accidental schema drift during deserialization.
- `UpdatePlanArgs` represents the full update request, bundling an optional `explanation` and an ordered list of `PlanItemArg`. The `plan` field is required, ensuring empty updates are rejected early.
- All types derive `Serialize`, `Deserialize`, `JsonSchema`, and `TS`, keeping Rust, JSON, and TypeScript definitions synchronized.

## Broader Context
- Consumed by the MCP plan tool and reflected back through `EventMsg::PlanUpdate` events in `protocol.rs`, enabling IDE integrations to display live plan progress. Specs covering plan orchestration should reference these types when describing the toolchain.
- The schema mirrors the TypeScript client in `codex-vscode/todo-mcp`; changes must remain compatible to avoid breaking editor integrations.
- Context can't yet be determined for richer plan metadata (e.g., assignees or timestamps); the struct can grow optional fields if those requirements emerge.

## Technical Debt
- None observed; the payloads match the existing MCP integration requirements.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
