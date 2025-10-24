## Overview
`core::tools::runtimes` groups concrete `ToolRuntime` implementations for apply_patch, shell, and unified exec requests. It also exposes a helper to build sandbox-ready command specifications shared across runtimes.

## Detailed Behavior
- Re-exports `apply_patch`, `shell`, and `unified_exec` modules that implement `ToolRuntime` for their respective tools.
- `build_command_spec` constructs a `CommandSpec` from tokenized commands, working directory, environment variables, timeouts, and escalation flags. It validates that at least one argument (the program) is present, returning `ToolError::Rejected` when commands are empty.
- Runtimes use `build_command_spec` to ensure consistent conversion into execution requests before delegating to `SandboxManager`.

## Broader Context
- The orchestrator calls into these runtimes after approvals and sandbox decisions are made. Consistent command specification building avoids duplication across runtimes.
- `CommandSpec` feeds into `SandboxAttempt::env_for`, connecting runtime requests to execute_env flows defined elsewhere in the crate.
- Context can't yet be determined for additional runtimes (e.g., git operations); new modules can be added alongside the existing ones without altering this helper.

## Technical Debt
- None observed; the module serves as an organizational hub and simple helper.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./apply_patch.rs.spec.md
  - ./shell.rs.spec.md
  - ./unified_exec.rs.spec.md
