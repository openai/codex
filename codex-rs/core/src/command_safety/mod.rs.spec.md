## Overview
The `command_safety` module aggregates the command classification helpers that gate shell execution. It centralizes exports for predicates that determine whether a command is allowed, warn-worthy, or blocked under Codex policies.

## Detailed Behavior
- Re-exports the unsafe-command detector in `is_dangerous_command`, allowing callers to short-circuit risky requests before handing them to the shell.
- Re-exports the allow-list matcher in `is_safe_command` so tools and policies can quickly confirm when a command is explicitly permitted.
- On Windows targets, surfaces platform-specific allowances via `windows_safe_commands`, which mirrors the Unix predicates while acknowledging OS limitations.
- Provides a single namespace for downstream modules (`core::command_safety`, `core::tools::handlers::shell`, rollout policies) to import when evaluating command strings, keeping platform branching scoped to this module.

## Broader Context
- Draws on configuration decisions documented in the crate-level spec (`../../mod.spec.md`) to ensure command execution aligns with rollout safety.
- Downstream handlers such as `core::tools::handlers::shell` and `core::command_safety::is_dangerous_command` rely on these exports for their decision trees, so any changes here ripple through CLI, TUI, and unified exec flows.

## Technical Debt
- None noted; this module remains a thin orchestrator over the platform-specific implementations.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../../mod.spec.md
  - ./is_safe_command.rs.spec.md
  - ./is_dangerous_command.rs.spec.md
  - ./windows_safe_commands.rs.spec.md
