## Overview
`process_hardening::lib` provides pre-main hardening routines for Codex binaries. The exported `pre_main_hardening` function is intended to run via `#[ctor::ctor]`, disabling dangerous features (core dumps, ptrace, preload env vars) before application code executes.

## Detailed Behavior
- `pre_main_hardening` dispatches to OS-specific helpers:
  - Linux/Android: `pre_main_hardening_linux` disables ptrace (`prctl(PR_SET_DUMPABLE, 0)`), sets the core size limit to zero, and removes environment variables starting with `LD_`.
  - macOS: `pre_main_hardening_macos` denies ptrace (`ptrace(PT_DENY_ATTACH)`), sets core size limit to zero, and removes `DYLD_` environment variables.
  - Windows: placeholder `pre_main_hardening_windows` (TODO).
- Shared helper `set_core_file_size_limit_to_zero` (unix-only) calls `setrlimit(RLIMIT_CORE, 0)`.
- Each OS-specific function logs errors to stderr and exits with a dedicated status code when hardening steps fail, preventing the application from running in an insecure state.

## Broader Context
- Codex binaries link this crate to enforce baseline security regardless of the calling environment. The hardening complements sandbox enforcement in `linux-sandbox` and approval policies in Codex core.
- Future Windows support will extend `pre_main_hardening_windows`; until then it is a no-op placeholder.

## Technical Debt
- Windows hardening is unimplemented; once requirements are known, this function should enforce analogous restrictions (e.g., disabling DLL injection).
- Environment variable scrubbing is limited to simple prefixes; configurable allow/deny lists could offer finer control if needed.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Implement Windows-specific hardening (disable core dumps, block debugger attach, strip risky env vars).
related_specs:
  - ../exec/src/lib.rs.spec.md
  - ../linux-sandbox/src/landlock.rs.spec.md
