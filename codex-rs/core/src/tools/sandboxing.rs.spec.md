## Overview
`core::tools::sandboxing` defines the traits and helpers that tool runtimes use to request approvals, manage sandbox attempts, and cache per-session decisions. It abstracts away the approval flow so the orchestrator can operate uniformly across different tool types.

## Detailed Behavior
- `ApprovalStore` keeps a session-scoped cache of `ReviewDecision`s keyed by serialized approval identifiers, allowing follow-up tool calls to reuse “ApprovedForSession” decisions. `with_cached_approval` consults the cache before invoking a provided async fetch.
- `ApprovalCtx` bundles the session, turn context, call ID, and optional retry reason presented to the user when requesting approval.
- `Approvable` defines how runtimes interact with approval logic:
  - `approval_key` identifies requests for caching.
  - `wants_escalated_first_attempt`, `should_bypass_approval`, and `wants_initial_approval` customize approval prompts based on policy and sandbox posture.
  - `start_approval_async` launches the async approval flow, returning the user’s decision.
- `SandboxablePreference` expresses a runtime’s sandbox affinity (`Auto`, `Require`, `Forbid`), with defaults allowing the orchestrator to automatically select a sandbox.
- `Sandboxable` and `ToolRuntime` extend the approval traits with execution hooks. `ToolRuntime::run` receives a `SandboxAttempt`, which carries the desired sandbox type, policy, manager, working directory, and optional seatbelt executable path.
- `SandboxAttempt::env_for` calls `SandboxManager::transform` to produce an `ExecEnv` tailored to the current sandbox selection.
- `ToolError` enumerates runtime failures (user rejection, sandbox denial with message, or wrapped `CodexErr`s).

## Broader Context
- Implementations in `runtimes/*` (shell, apply_patch, unified_exec) implement `ToolRuntime`, leveraging these traits to integrate with the orchestrator in `orchestrator.rs`.
- Approval caching is keyed by serialized structs; keeping keys stable across releases is important to avoid accidental cache misses or mismatched approvals.
- Context can't yet be determined for per-tool approval persistence beyond the session; if added, the cache might need to persist decisions to disk.

## Technical Debt
- `SandboxablePreference::Require`/`Forbid` are currently unused; clarifying their future integration or removing them would reduce confusion.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Document or implement use of `SandboxablePreference::Require/Forbid` to avoid dead-code allowances.
related_specs:
  - ./orchestrator.rs.spec.md
  - ./runtimes/mod.rs.spec.md
  - ../sandboxing/mod.spec.md
