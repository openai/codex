## Overview
`landlock` runs the Codex Linux sandbox binary end to end, verifying filesystem restrictions, timeout handling, and outbound network blocking on Linux hosts.

## Detailed Behavior
- Helper `run_cmd` executes commands under the sandbox using `process_exec_tool_call` with workspace-write policies and configurable writable roots/timeouts. It prints stdout/stderr before panicking when commands fail.
- Tests cover:
  - Reading system directories (`test_root_read`) and preventing writes without writable roots (`test_root_write`).
  - Writing to `/dev/null` and to whitelisted temp directories.
  - Timeout enforcement via short `sleep` calls.
  - Network prohibition across a variety of binaries (`curl`, `wget`, `ping`, `nc`, `ssh`, `getent`, `/dev/tcp` redirection). `assert_network_blocked` accepts non-zero exit codes while tolerating missing binaries (exit 127).
- Timeouts differ on `aarch64` to accommodate slower CI environments.
- Utilities rely on `ShellEnvironmentPolicy::default()` to mirror production env setup.

## Broader Context
- Provides regression coverage for `codex-linux-sandbox` and core execution policies, complementing specs in Phase 1 describing sandbox enforcement.

## Technical Debt
- Tests currently assume specific binaries exist; adding guards for absent tools (beyond exit 127) or using mock binaries could reduce flakiness on minimal systems.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Harden network-denial tests to gracefully skip when required binaries are unavailable, avoiding flakiness on stripped-down CI images.
related_specs:
  - ../../mod.spec.md
  - ./mod.rs.spec.md
  - ../../../linux-sandbox/src/main.rs.spec.md
