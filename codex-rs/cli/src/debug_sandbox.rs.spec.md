## Overview
`codex-cli::debug_sandbox` runs user-provided commands inside Codex’s seatbelt (macOS) or landlock (Linux) sandboxes. It mirrors the execution pipeline used by Codex when approving shell commands, giving developers a way to test sandbox behavior from the CLI.

## Detailed Behavior
- Public entrypoints:
  - `run_command_under_seatbelt` / `run_command_under_landlock` accept the corresponding `SeatbeltCommand` or `LandlockCommand` (from `lib.rs`) and delegate to `run_command_under_sandbox` with the desired sandbox type.
- `run_command_under_sandbox`:
  - Resolves `SandboxMode` via `create_sandbox_mode` (`--full-auto` → `WorkspaceWrite`, otherwise `ReadOnly`).
  - Loads configuration with CLI overrides (`Config::load_with_cli_overrides`), supplying sandbox mode and optional `codex_linux_sandbox_exe`.
  - Derives the working directory and sandbox-policy cwd from `Config`.
  - Creates an `ExecEnv` using `create_env` (respecting shell environment policy).
  - Depending on sandbox type:
    - Seatbelt: calls `codex_core::seatbelt::spawn_command_under_seatbelt`.
    - Landlock: ensures the Linux sandbox binary is available (`config.codex_linux_sandbox_exe`) and calls `codex_core::landlock::spawn_command_under_linux_sandbox`.
  - Waits for the child process and hands the exit status to `exit_status::handle_exit_status`, which exits the process with the child’s code or signal.
- `create_sandbox_mode` helper encodes the CLI’s `--full-auto` flag into a `SandboxMode`.

## Broader Context
- Used by `codex sandbox` subcommands in `main.rs`. These helpers reuse Codex’s core sandboxing implementations to provide parity with automated tool runs.
- Windows is unsupported (seatbelt/landlock are platform-specific); CLI guards ensure the subcommands only appear where meaningful.
- Exit handling funnels through `exit_status.rs`, keeping behavior consistent with other command wrappers.

## Technical Debt
- Sandbox cwd currently mirrors `config.cwd`; offering separate CLI flags for command cwd vs policy cwd could improve flexibility.
- Better diagnostics (e.g., surfacing sandbox setup errors before exit) would help users debug configuration issues.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Allow specifying distinct command/sandbox cwds to mirror future runtime capabilities.
    - Improve error reporting around sandbox setup failures to avoid silent exits.
related_specs:
  - ./lib.rs.spec.md
  - ./exit_status.rs.spec.md
  - ../core/src/sandboxing/mod.rs.spec.md
