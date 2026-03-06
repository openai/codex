# Exomind Governance Runtime MVP (M5)

## Goal

Connect governance checks to local Codex session execution points:

- after generation (`after_agent`)
- before write/tool execution (`before_tool_use`)

## Hook Integration

Runtime integration is wired through `codex-hooks` and `codex-core`:

- `HookEvent::AfterAgent` for generation-complete checks.
- `HookEvent::BeforeToolUse` for pre-execution checks (shell/apply_patch).
- `HookEvent::AfterToolUse` remains available for post-execution telemetry.

## Enablement

Set environment variables before launching Codex:

- `EXOMIND_NORM_GOVERNANCE_ENABLED=1`
- `EXOMIND_NORM_GOVERNANCE_MODE=warn|block`
- `EXOMIND_NORM_GOVERNANCE_CATALOG=<path>` (optional, default `docs/exomind-rule-catalog-template.json`)
- `EXOMIND_NORM_GOVERNANCE_WAIVERS=<path>` (optional)

## Decision Model

- `warn` mode:
  - violations return continueable hook failures (operation continues).
- `block` mode:
  - unwaived L1 or `action=block` violations abort operation before tool execution.

## Evidence Payload

Each finding emits a normalized evidence object:

- `stage`
- `rule_id`, `rule_level`, `severity`, `action`
- `trigger`
- `snippet`
- `waived`, `waiver_id`, `waiver_owner`, `waiver_reason`

## Current Rule Coverage (MVP)

- `L1-SEC-NO-SHELL-UNSAFE`: shell pre-check heuristic.
- `L2-TEST-CHANGED-CODE-HAS-TEST`: apply_patch pre-check heuristic.
- `L3-STYLE-IMPORT-ORDER`: assistant output post-check heuristic.
