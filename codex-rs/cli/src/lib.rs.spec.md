## Overview
`codex-cli::lib` exposes helper CLI argument structs used by the top-level `codex` binary and its sandbox subcommands. It defines the seatbelt and landlock wrappers that run commands in platform-specific sandboxes and re-exports the login module.

## Detailed Behavior
- Modules:
  - `debug_sandbox`: async helpers that spawn commands under Seatbelt (macOS) or Landlock (Linux).
  - `exit_status`: cross-platform exit-code handling for spawned sandboxed commands.
  - `login`: login/logout flows shared with `main.rs`.
- CLI argument structs:
  - `SeatbeltCommand` / `LandlockCommand` (`clap::Parser`) capture `--full-auto`, configuration overrides (`CliConfigOverrides`), and the trailing command vector. These are used by `codex cli sandbox seatbelt|landlock`.
- `pub use` exports (`debug_sandbox`, `login`) allow the binary to call into sandbox and login helpers without referencing module paths directly.

## Broader Context
- `codex-rs/cli/src/main.rs` builds on these structs when wiring the sandbox subcommand.
- Sandbox helpers depend on core configuration (`codex_core`) to derive policies before calling `codex_core::seatbelt` or `landlock`. See `debug_sandbox.rs.spec.md` for execution details.
- Context can't yet be determined for additional sandbox variants; future additions would mirror these patterns with new command structs.

## Technical Debt
- None observed; the module is primarily a façade.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./debug_sandbox.rs.spec.md
  - ./login.rs.spec.md
  - ./main.rs.spec.md
