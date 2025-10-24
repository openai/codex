## Overview
`core::tools::runtimes::shell` executes shell commands through the orchestrator. It integrates command safety heuristics, approval caching, and sandbox execution to provide a consistent experience for tools that run shell commands.

## Detailed Behavior
- `ShellRequest` carries the command vector, working directory, timeout, environment, and optional escalation parameters.
- `ShellRuntime` implements `Sandboxable` (auto preference, escalation allowed) and `Approvable`:
  - Approval keys include the command, working directory, and whether the request asks for escalated permissions.
  - `start_approval_async` uses `with_cached_approval` to reuse decisions and calls `Session::request_command_approval`, passing through custom justifications or retry reasons.
  - `wants_initial_approval` applies heuristics: known-safe commands skip approval, `UnlessTrusted` always prompts, and `OnRequest` prompts when commands seem dangerous or request escalation. Danger evaluation leverages `command_might_be_dangerous` and the active `SandboxPolicy`.
  - `wants_escalated_first_attempt` mirrors the request’s escalation flag.
- `ToolRuntime::run` builds a `CommandSpec` via `build_command_spec`, transforms it into an `ExecEnv` using the current `SandboxAttempt`, and executes the command with `execute_env`, streaming stdout via the session.

## Broader Context
- Shell execution is the most common tool path; maintaining clear approval heuristics ensures users aren’t overwhelmed with prompts while still protecting against dangerous commands.
- The orchestrator handles fallback to no-sandbox attempts and additional approvals; this runtime focuses on building the env and invoking exec.
- Context can't yet be determined for fine-grained command policies; future work may extend `command_might_be_dangerous` or the approval key to include additional metadata.

## Technical Debt
- None observed; heuristics rely on existing command safety modules, and approval caching keeps flows efficient.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
  - ../sandboxing.rs.spec.md
  - ../../command_safety/is_safe_command.rs.spec.md
