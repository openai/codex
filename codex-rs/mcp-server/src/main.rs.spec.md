## Overview
The MCP server binary mirrors other Codex binaries: it bootstraps through the `arg0` wrapper and delegates execution to the crate’s `run_main` entrypoint.

## Detailed Behavior
- Calls `arg0_dispatch_or_else`, which handles potential sandbox relaunches before running the async block.
- Invokes `run_main` with the optional `codex-linux-sandbox` path and default CLI overrides, returning any `anyhow::Error` back to the OS.

## Broader Context
- Keeps the executable thin so operational logic resides in `codex_mcp_server::run_main` (`./lib.rs.spec.md`).
- Matches the startup contract used by the CLI and app server for consistent sandbox expectations.

## Technical Debt
- None; the entrypoint intentionally delegates all complex work.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./lib.rs.spec.md
  - ../../app-server/src/main.rs.spec.md
