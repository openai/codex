## Overview
`core::seatbelt` wraps Apple’s `sandbox-exec` (“seatbelt”) to apply macOS sandbox policies. It prepares command-line arguments and environment variables so shell/apply-patch executions respect the active `SandboxPolicy`, denying writes outside explicit roots and optionally blocking network access.

## Detailed Behavior
- `MACOS_SEATBELT_BASE_POLICY` embeds the base SBPL policy (from `seatbelt_base_policy.sbpl`). All generated policies extend this base.
- `spawn_command_under_seatbelt`:
  - Builds the seatbelt argument list via `create_seatbelt_command_args`.
  - Injects `CODEX_SANDBOX_ENV_VAR=seatbelt` and delegates to `spawn_child_async` to launch `/usr/bin/sandbox-exec`, ensuring no path injection occurs.
- `create_seatbelt_command_args`:
  - Computes writable root policies from `SandboxPolicy::get_writable_roots_with_cwd`, canonically resolving roots and automatically marking `.git` subpaths as read-only.
  - When writable roots exist, generates `(allow file-write* ...)` clauses with per-root parameters and `require-not` guards for read-only subpaths.
  - Adds read-only, network, and system socket permissions based on the policy (`has_full_disk_read_access`, `has_full_network_access`).
  - Concatenates the base policy, read/write/network clauses, and finishes with `--` followed by the original command.
  - Returns additional `-DNAME=VALUE` parameters that sandbox-exec uses inside the SBPL template.
- Tests confirm:
  - `.git` directories remain read-only even when the repo root is writable.
  - Default writable roots (cwd, `/tmp`, `$TMPDIR`) are handled correctly and parameters are stable.
- `MACOS_PATH_TO_SEATBELT_EXECUTABLE` is hardcoded to `/usr/bin/sandbox-exec`, preventing path spoofing by untrusted environments.

## Broader Context
- `SandboxManager` uses this module when `SandboxType::MacosSeatbelt` is selected. Ensuring the generated SBPL accurately reflects writable roots keeps apply_patch and shell commands aligned with user policies.
- The policy template in `seatbelt_base_policy.sbpl` defines baseline restrictions; this module adds dynamic clauses per session.
- Context can't yet be determined for seatbelt deprecation (Apple removed `sandbox-exec` in newer macOS versions); future work may need a replacement mechanism.

## Technical Debt
- None specified; future macOS changes may require revisiting the hardcoded sandbox-exec path or policy format.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Monitor macOS support for `sandbox-exec` and plan a replacement once the tool is deprecated on newer releases.
related_specs:
  - ./sandboxing/mod.rs.spec.md
  - ./spawn.rs.spec.md
  - ./exec.rs.spec.md
