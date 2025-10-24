## Overview
`codex-tui::lib` orchestrates the interactive terminal client. It parses CLI overrides, loads configuration, initializes telemetry/logging, runs onboarding checkpoints, and launches the ratatui event loop via `run_ratatui_app`. The module re-exports key CLI types (`Cli`, `AppExitInfo`, public widgets) and houses the top-level async `run_main` entrypoint.

## Detailed Behavior
- Module graph:
  - Declares submodules for the entire TUI stack (chat widgets, rendering, onboarding, updates, etc.) and re-exports commonly consumed pieces (`Cli`, `ComposerInput`, etc.).
- `run_main(cli, codex_linux_sandbox_exe)`:
  - Derives sandbox and approval policies from CLI flags (`--full-auto`, `--dangerously-bypass-approvals-and-sandbox`) and optional overrides (`--sandbox`, `--ask-for-approval`).
  - Applies OSS-specific configuration (ensuring local OSS models, forcing provider overrides).
  - Canonicalizes CWD and additional writable directories; validates with `additional_dirs::add_dir_warning_message`.
  - Builds `ConfigOverrides` and parses root-level `-c` overrides (`CliConfigOverrides`), exiting with a message on parse failure.
  - Loads the config via `load_config_or_exit`, enforces login restrictions, and initializes logging:
    - Creates a log file (`codex-tui.log`) with restricted permissions (chmod 600 on Unix).
    - Installs tracing layers: file logging, Codex feedback writer, and optional OTEL exporter.
  - Ensures OSS models are pull-ready when `--oss` is set.
  - Calls `run_ratatui_app`, mapping its `color_eyre::Result<AppExitInfo>` into `std::io::Result`.
- `run_ratatui_app`:
  - Installs `color_eyre` and a panic hook that forwards panics to tracing.
  - Initializes the ratatui terminal (`tui::init`), clearing the screen, and constructs a `Tui`.
  - On non-debug builds, optionally runs the update prompt UI before entering the main app, allowing the user to defer or accept updates.
  - Initializes high-fidelity session logging.
  - Collects login status, trust warnings, and determines whether onboarding screens should appear. If onboarding is needed:
    - Runs `onboarding::run_onboarding_app`, reloading config when the user grants trust/WSL acknowledgments.
    - Handles Windows WSL instructions (restoring the terminal and printing guidance) when chosen.
  - Computes resume behavior (`ResumeSelection`) by checking CLI resume flags (`--last`, session ID) or defaulting to start fresh.
  - Initializes `AuthManager`, `App`, and `codex_feedback::CodexFeedback`, then delegates to `App::run`.
  - Restores the terminal and returns `AppExitInfo` with token usage, conversation ID, and optional update action.
- Helper functions:
  - `load_config_or_exit` reloads config with overrides (used after onboarding).
  - `get_login_status`, `should_show_onboarding`, `should_show_trust_screen`, etc., determine onboarding flows.

## Broader Context
- CLI layer (`codex-rs/cli/src/main.rs`) calls `run_main`. This module ensures config, logging, and telemetry setup align with non-interactive flows (`codex_exec`), sharing login and sandbox logic.
- `App::run` (see `app.rs.spec.md`) drives the event loop; `tui::Tui` manages terminal primitives. Onboarding screens and updates reuse modules under `onboarding` and `updates`.
- Context can't yet be determined for Windows support; code contains WSL messaging but broader Windows functionality is still limited.

## Technical Debt
- `run_main` combines configuration, telemetry, onboarding, and terminal setup in one function; factoring into smaller helpers would aid maintainability.
- Error handling often exits via `std::process::exit`; returning structured errors would make integration tests easier.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Break `run_main`/`run_ratatui_app` into smaller helpers (config, telemetry, onboarding) to simplify modification.
    - Replace direct `process::exit` calls with error propagation where possible to improve testability.
related_specs:
  - ./app.rs.spec.md
  - ./tui.rs.spec.md
  - ./updates.rs.spec.md
  - ../core/src/config.rs.spec.md
