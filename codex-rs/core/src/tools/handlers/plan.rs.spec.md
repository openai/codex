## Overview
`core::tools::handlers::plan` implements the `update_plan` tool, allowing the model to publish step-by-step plans. Rather than executing commands, it emits `PlanUpdate` events so clients can render the plan and monitor status.

## Detailed Behavior
- Accepts only `ToolPayload::Function`; other payloads trigger model-facing errors.
- `handle_update_plan` parses JSON into `UpdatePlanArgs` (explanation + vector of `{step, status}` items) and sends an `EventMsg::PlanUpdate` through the session’s event channel. The function returns `"Plan updated"` as a simple acknowledgement.
- Validation relies on serde: malformed JSON yields `FunctionCallError::RespondToModel("failed to parse function arguments...")`.
- `PLAN_TOOL` exposes a reusable `ToolSpec` describing the schema, ensuring the same tool definition is used both for registration and tests.

## Broader Context
- Front-end clients use `PlanUpdate` events to present plan state, often pausing between directories per the documentation plan. The tool aids observational visibility rather than performing actions.
- Because the handler does not mutate session state beyond emitting events, it is safe to call multiple times per turn.
- Context can't yet be determined for plan archiving; future enhancements might store history or enforce stronger validation of statuses.

## Technical Debt
- None flagged; flow is intentionally simple.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../../protocol/src/plan_tool.rs.spec.md
