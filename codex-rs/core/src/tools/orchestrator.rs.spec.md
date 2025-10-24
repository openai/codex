## Overview
`core::tools::orchestrator` coordinates sandbox selection, approval prompts, and retry semantics for tool runtimes. It centralizes the logic so shell, apply-patch, and future runtimes can share consistent approval flows and escalation behavior.

## Detailed Behavior
- `ToolOrchestrator` owns a `SandboxManager` and exposes `run`, which accepts a runtime (`ToolRuntime`), request payload, tool context, turn context, and the session’s approval policy.
- The method proceeds in stages:
  1. **Approval**: Calls `tool.wants_initial_approval` to determine whether a pre-execution approval is required. When requested, it invokes `tool.start_approval_async`, logs the decision via OTEL, and aborts if the user denies or aborts the request. Otherwise, approval is recorded and reused for later retries.
  2. **Initial sandbox attempt**: Chooses the sandbox mode based on the turn’s `SandboxPolicy` and the runtime’s `sandbox_preference`. If the runtime wants escalated permissions on the first attempt, it bypasses sandboxing. The runtime’s `run` method is invoked with a `SandboxAttempt` describing the sandbox context.
  3. **Sandbox denial handling**: If the sandbox denies execution (timeout/permission issues) and the runtime allows escalation, the orchestrator:
     - Honors `AskForApproval::Never` by returning a standardized denial message (`build_never_denied_message_from_output`) without retrying.
     - Otherwise, optionally prompts for approval again with a retry reason derived from the original output (`build_denial_reason_from_output`) unless the runtime opts to bypass approval.
     - Retries the tool with `SandboxType::None`, omitting the sandbox executable path.
  4. Other errors—interruptions, fatal errors, or usage limit responses—bubble up without additional handling, letting callers differentiate between error classes.
- Helper functions translate sandbox-denied outputs into concise messages for approval prompts and final errors.

## Broader Context
- `handle_container_exec_with_params` in `tools/mod.rs` uses the orchestrator for both shell and apply-patch requests. Implementations of `ToolRuntime` (see `runtimes/*`) specify approval preferences, sandbox behavior, and output packaging.
- The orchestrator interacts with the telemetry layer (`OtelEventManager`) to record tool decisions, ensuring audits capture user vs. config-driven approvals.
- Context can't yet be determined for multi-step retries or parallel sandbox attempts; the current design is linear (sandboxed attempt → optional escalated retry).

## Technical Debt
- None observed; the module encapsulates approval/sandbox orchestration with clear entry points.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ./sandboxing.rs.spec.md
  - ./runtimes/mod.rs.spec.md
