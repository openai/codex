## Overview
`codex_tui::lib` exposes the public surface for the terminal UI, coordinates configuration/runtime setup, and launches the main application loop. It re-exports key widgets for integration tests and external consumers.

## Detailed Behavior
- Module declarations bring in all TUI subsystems (app state, widgets, streaming, onboarding, etc.). Public exports include:
  - `Cli`, `ComposerInput`, `ComposerAction`, `render_markdown_text`, and `insert_history` helpers.
  - `AppExitInfo` for callers to inspect session results (token usage).
- `run_main(cli, codex_linux_sandbox_exe)` orchestrates:
  - CLI flag normalization (`--oss`, sandbox/approval combinations, additional directories).
  - Parsing raw `-c` overrides and loading `Config` via `load_config_or_exit`.
  - Enforcing login restrictions (exiting with an error when violated) and warning when `--add-dir` is ignored.
  - Initializing logging (`codex-tui.log`) with non-blocking append and optional OpenTelemetry bridge.
  - Optionally running onboarding flows (including trust directory selection) and showing WSL instructions when appropriate.
  - Creating the TUI (`tui::init`), `App`, and main event loop via `app.run`.
  - Persisting session logs/rollouts, updating active profile metadata, and handling resume logic via `find_conversation_path_by_id_str`.
- Helper functions:
  - `load_config_or_exit` loads config with overrides, printing errors and exiting when parsing fails.
  - `maybe_prompt_trust_directory` triggers onboarding UI if the configured workspace requires trust confirmation.
  - `setup_logging` wires `tracing_subscriber` with `EnvFilter` and optional OTLP exporter.

## Broader Context
- The binary entrypoint (`src/main.rs`) simply parses CLI args and calls `run_main`. Integration tests reuse `run_main` to drive the application, while exported widgets power component-level testing.

## Technical Debt
- `run_main` mixes configuration, logging, onboarding, and TUI startup in one function; future refactors could split these responsibilities (config bootstrap, logging init, UI loop) for clarity and targeted testing.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Factor configuration/bootstrap logic into dedicated functions or modules to reduce `run_main` complexity and improve testability.
related_specs:
  - ./main.rs.spec.md
  - ./app.rs.spec.md
  - ./tui.rs.spec.md
