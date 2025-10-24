## Overview
`core::landlock` integrates with the `codex-linux-sandbox` helper to enforce sandbox policies on Linux using Landlock and seccomp. It translates Codex’s `SandboxPolicy` into CLI arguments and launches commands under the sandbox wrapper.

## Detailed Behavior
- `spawn_command_under_linux_sandbox`:
  - Accepts the path to the sandbox executable, the command to run, working directory, sandbox policy, stdio policy, and environment variables.
  - Calls `create_linux_sandbox_command_args` to generate CLI options, sets `arg0` to `codex-linux-sandbox`, and delegates to `spawn_child_async` so spawn semantics remain consistent with other platforms.
- `create_linux_sandbox_command_args` serializes the policy to JSON (`--sandbox-policy`) and records the sandbox CWD (`--sandbox-policy-cwd`). It appends `--` followed by the original command to avoid accidental flag parsing.
- The helper relies on the external `codex-linux-sandbox` binary to apply the Landlock/ seccomp rules described by the JSON policy. By forwarding the full policy, the sandbox can enforce the same writable roots, network restrictions, and sandbox semantics as macOS seatbelt.

## Broader Context
- `SandboxManager` invokes this module when sandboxing on Linux, keeping command invocation consistent with macOS seatbelt handling. Execution output is still captured by `exec.rs`.
- The JSON representation mirrors the server-facing policy definitions; any changes to `SandboxPolicy` serialization must keep this API stable.
- Context can't yet be determined for richer CLI options (e.g., debug logging); future updates to `codex-linux-sandbox` may require extending argument generation here.

## Technical Debt
- None noted; behaviour is straightforward, assuming the helper binary is available.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./sandboxing/mod.rs.spec.md
  - ./spawn.rs.spec.md
  - ./exec.rs.spec.md
