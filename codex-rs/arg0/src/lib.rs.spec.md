## Overview
`lib.rs` implements `arg0_dispatch_or_else`, the entrypoint wrapper used by Codex binaries. It inspects the invoked executable name (`argv[0]`) to dispatch to sandbox or apply_patch subcommands, loads `.env` overrides, adjusts `PATH`, and finally runs the async main function with optional sandbox paths.

## Detailed Behavior
- Dispatch flow:
  - If invoked as `codex-linux-sandbox`, call `codex_linux_sandbox::run_main()` (never returns).
  - If invoked as `apply_patch`/`applypatch`, call `codex_apply_patch::main()`.
  - Handles the secret `--codex-run-as-apply-patch` path to execute `codex_apply_patch::apply_patch` directly.
- Environment setup:
  - `load_dotenv` loads `~/.codex/.env`, filtering out variables starting with `CODEX_` for security.
  - `prepend_path_entry_for_apply_patch` creates a temp dir containing symlinks (or batch scripts on Windows) pointing to the current executable, prepends it to `PATH`, and keeps the `TempDir` alive for the process duration.
- Runtime creation:
  - Builds a tokio runtime and calls `main_fn(codex_linux_sandbox_exe)` where the sandbox path is provided on Linux via `current_exe()`.
- Utility constants handle command names and filtering.

## Broader Context
- All CLI binaries in the workspace wrap their `main` function with this helper to keep deployment as a single binary while supporting multiple subcommands and helper tools.

## Technical Debt
- None; TODO comments note potential improvements (e.g., Windows links).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../apply-patch/src/lib.rs.spec.md
