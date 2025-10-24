## Overview
`core::tools::runtimes::apply_patch` executes verified apply_patch requests under the orchestrator. It reuses the cached approval decision (including explicit user approvals) and runs the Codex binary in apply-patch mode with a minimal environment.

## Detailed Behavior
- `ApplyPatchRequest` bundles the patch text, working directory, timeout, explicit-approval flag, and optional codex executable path (used when running inside seatbelt).
- `ApplyPatchRuntime::build_command_spec` constructs a `CommandSpec` pointing to the Codex binary with `CODEX_APPLY_PATCH_ARG1` and the patch contents, ensuring the environment is empty for deterministic behavior.
- `stdout_stream` forwards incremental stdout to the session so UIs receive live updates during execution.
- Implements `Sandboxable` (auto preference, escalation allowed) and `Approvable`:
  - Approval key combines patch text and working directory so repeated attempts in the same session can reuse decisions.
  - `start_approval_async` uses `with_cached_approval` to avoid re-prompting when the user explicitly approved earlier or when retrying after a sandbox denial (passing the retry reason to the approval request).
- `ToolRuntime::run` builds the command spec, transforms it into an `ExecEnv` via the current `SandboxAttempt`, and executes it with `execute_env`, mapping errors into `ToolError::Codex`.

## Broader Context
- Upstream verification (`maybe_parse_apply_patch_verified`) ensures only safe patches reach this runtime. The orchestrator handles sandbox retries and approval prompts; this runtime focuses on constructing the execution environment.
- Rollout diff tracking is managed outside the runtime (in `handle_container_exec_with_params` and event emitters).
- Context can't yet be determined for streaming patch previews or dry-run modes; those would extend the request struct and runtime logic.

## Technical Debt
- None observed; responsibilities are well-scoped and rely on shared helpers.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../sandboxing.rs.spec.md
  - ../../codex.rs.spec.md
