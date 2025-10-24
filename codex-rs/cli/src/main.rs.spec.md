## Overview
`codex-cli::main` implements the `codex` multitool binary. It routes between interactive TUI, non-interactive exec, MCP tools, sandbox wrappers, and utility commands. It also handles feature toggles, configuration overrides, and update prompts.

## Detailed Behavior
- CLI structure (`MultitoolCli`):
  - Flattens `CliConfigOverrides` and `FeatureToggles` so root-level flags (e.g., `-c key=value`, `--enable feature`) propagate to subcommands.
  - Defaults to launching the interactive TUI (`codex_tui::run_main`) when no subcommand is provided.
- Subcommands:
  - `Exec`: runs `codex_exec::run_main`.
  - `Login`, `Logout`: call into `login.rs` helpers for OAuth/API key workflows.
  - `Sandbox`: wraps seatbelt/landlock execution via `debug_sandbox`.
  - `Mcp`, `McpServer`: manage MCP server configs or run the MCP server.
  - `AppServer`, `Cloud`, `ResponsesApiProxy`, `StdioToUds`, `GenerateTs`: forward to respective crates.
  - `Apply`: runs the latest diff via `codex_chatgpt::apply_command`.
  - `Resume`: rehydrates TUI state for a previous session with options to pick last session ID (`codex_tui` resume flags).
  - `Features`: lists feature flags using `Config::load_with_cli_overrides`.
  - `Completion`: generates shell completions via `clap_complete`.
- Config override management:
  - `prepend_config_flags` ensures root-level overrides are prepended to subcommand-specific override lists, giving subcommands precedence.
  - Feature toggles translate into `features.<name>=true|false` overrides.
- Event loop helpers:
  - `cli_main` handles dispatch; `handle_app_exit` prints token usage/resume hints and optionally runs update actions from `codex_tui`.
  - `format_exit_messages` adds resume instructions and respects color output.
  - `run_update_action` executes upgrade commands suggested by the TUI.
- Integration points:
  - `arg0_dispatch_or_else` enables dual binary behavior (e.g., `codex-linux-sandbox`).
  - `#[ctor::ctor]` hardens the process via `codex_process_hardening`.

## Broader Context
- This module is the user-facing entrypoint; ensuring cross-subcommand override consistency keeps behavior aligned with `codex_exec`, `codex_tui`, and other components.
- Login flows (`login.rs.spec.md`) and sandbox helpers (`debug_sandbox.rs.spec.md`) share configuration logic.
- Context can't yet be determined for multi-platform differences beyond sandbox subcommands; future additions should continue to centralize override handling.

## Technical Debt
- `cli_main` is sizable and blends override manipulation, dispatch, and output formatting; splitting into per-subcommand handlers would improve readability.
- Resume logic and config merging are complex; documenting precedence rules (possibly in docs) would aid contributors.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor `cli_main` into smaller functions to simplify maintenance and testing.
    - Document/encapsulate override precedence to reduce the risk of regressions when adding new subcommands.
related_specs:
  - ./lib.rs.spec.md
  - ./login.rs.spec.md
  - ./debug_sandbox.rs.spec.md
  - ../exec/src/lib.rs.spec.md
