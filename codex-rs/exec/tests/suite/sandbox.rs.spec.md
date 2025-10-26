## Overview
`sandbox` validates that the exec crate’s sandbox launchers behave correctly across macOS Seatbelt and Linux Landlock implementations.

## Detailed Behavior
- Provides platform-specific `spawn_command_under_sandbox` helpers that delegate to either `spawn_command_under_seatbelt` (macOS) or `spawn_command_under_linux_sandbox` (Linux).
- Key scenarios:
  - `python_multiprocessing_lock_works_under_sandbox` ensures Python multiprocessing primitives operate within sandboxed environments.
  - `sandbox_distinguishes_command_and_policy_cwds` checks that writable roots are respected relative to the sandbox cwd, not the command cwd.
  - `allow_unix_socketpair_recvfrom` (and helper `unix_sock_body`) confirm Unix domain socket operations remain allowed.
  - Additional helpers verify network-denied conditions (curl/wget/etc.), writable root enforcement, `/dev/null` access, and timeout behavior.
- Common helper `run_code_under_sandbox` re-executes the current test binary under the sandbox with `IN_SANDBOX` to separate parent/child logic.

## Broader Context
- Ensures the sandbox integration described in Phase 1 remains functional across operating systems, preventing regressions in network lockdown or filesystem protections.

## Technical Debt
- Tests duplicate setup logic with the `linux-sandbox` crate; consolidating sandbox test utilities across crates could improve maintainability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Share sandbox re-exec helpers across exec and linux-sandbox suites to avoid divergence.
related_specs:
  - ../mod.spec.md
  - ../../../linux-sandbox/tests/suite/landlock.rs.spec.md
  - ../../src/lib.rs.spec.md
