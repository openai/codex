## Overview
`standalone_executable.rs` provides the `apply_patch` binary entrypoint used outside the Codex runtime. It parses command-line arguments, reads patch bodies, and delegates to the library’s `apply_patch` function with stdout/stderr wiring.

## Detailed Behavior
- `main` calls `run_main` and exits with its return code (using `std::process::exit` because `ExitCode::exit_process` is nightly).
- `run_main`:
  - Accepts either a single argument containing the patch or reads from stdin when no argument is supplied.
  - Validates UTF-8 inputs, prints usage/helpful errors for missing or extra arguments, and returns distinct exit codes (`1` for IO errors, `2` for usage issues).
  - Delegates to `crate::apply_patch`, forwarding stdout/stderr mutably; flushes stdout on success to keep pipeline ordering predictable.
- Uses standard IO traits so the integration mirrors how Codex tool execution captures outputs.

## Broader Context
- Enables developers to test patches locally (`cargo run -p codex-apply-patch -- <patch>`).
- The crate’s `main.rs` simply calls this entrypoint so packaging the binary requires no extra wiring.

## Technical Debt
- None; argument handling and error codes align with Unix conventions.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
