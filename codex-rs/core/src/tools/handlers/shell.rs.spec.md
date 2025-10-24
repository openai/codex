## Overview
`core::tools::handlers::shell` adapts both function-style and local shell tool payloads into `ExecParams` and forwards them to the shared execution pipeline. It ensures commands inherit the turn’s sandbox configuration, environment policy, and working directory before invoking `handle_container_exec_with_params`.

## Detailed Behavior
- Accepts `ToolPayload::Function` (JSON arguments parsed into `ShellToolCallParams`) or `ToolPayload::LocalShell` (already parsed by the router). Other payloads yield a model-facing error.
- `to_exec_params` resolves the working directory against the `TurnContext`, builds the environment via `create_env` using the turn’s configured shell environment policy, and propagates timeout, escalation, and justification fields.
- `handle` clones the session, turn, and diff tracker arcs, invokes `handle_container_exec_with_params`, and wraps the returned formatted output in `ToolOutput::Function { success: Some(true) }`.
- Local shell invocations originate from streaming Responses API events, while function payloads are produced by function-calling models; both paths share the same execution flow after parameter parsing.

## Broader Context
- The router directs `FunctionCall` payloads to this handler when the tool name matches the configured shell tool. The runtime plumbing (approval, sandbox retries, telemetry) occurs within `handle_container_exec_with_params` and the orchestrator.
- Environment creation respects sandbox policies; any change to environment inheritance rules should update `create_env` and thus this handler’s behavior.
- Context can't yet be determined for custom shells beyond `ShellToolCallParams`; additional fields would require extending the parameter struct and parser.

## Technical Debt
- None identified in this module; approval heuristics live in the shell runtime.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
