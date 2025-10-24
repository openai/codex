## Overview
`codex-cli::login` implements headless login, logout, and status flows for the Codex CLI. It supports ChatGPT OAuth (browser or device code), API key authentication, and workspace-specific login restrictions.

## Detailed Behavior
- Configuration loading:
  - `load_config_or_exit` parses CLI config overrides, loads `Config::load_with_cli_overrides`, and exits with an error message on failure.
- ChatGPT login:
  - `run_login_with_chatgpt` validates `ForcedLoginMethod`, reads config, and delegates to `login_with_chatgpt`, which starts the local login server (`codex_login::run_login_server`) using `ServerOptions`.
  - On success, prints a confirmation and exits 0; on error, exits 1.
  - `run_login_with_device_code` performs OAuth device code flow via `run_device_code_login`, allowing optional issuer/client overrides.
- API key login:
  - `run_login_with_api_key` enforces method restrictions, reads the key (via `read_api_key_from_stdin` if `--with-api-key`), and calls `codex_core::auth::login_with_api_key`. Outputs success/failure messages and exits accordingly.
  - `read_api_key_from_stdin` ensures the key is piped (not from a TTY), trims whitespace, and terminates with guidance on misuse.
- Status/logout:
  - `run_login_status` loads config and inspects `CodexAuth::from_codex_home`. It reports ChatGPT vs API key mode, including masked key output via `safe_format_key`.
  - `run_logout` removes stored credentials (`codex_core::auth::logout`), reporting success or absence.
- Helpers:
  - `login_with_chatgpt` (async) prints server details and waits for completion.
  - `safe_format_key` masks long keys (`prefix***suffix`) and returns `***` for short ones.
- Process exits (`std::process::exit`) are used to provide shell-friendly semantics.

## Broader Context
- `main.rs` routes login-related subcommands here. Authentication state is consumed by other binaries (`codex_exec`, `codex_tui`) through shared config/auth modules.
- Forced login methods (`ForcedLoginMethod`) respect enterprise policies configured in `config.toml`.
- Context can't yet be determined for future auth providers; new flows should reuse the configuration helpers established here.

## Technical Debt
- Functions call `std::process::exit` directly; restructuring into Result-returning APIs would simplify testing.
- Device-code and server login share logic; extracting shared helpers (e.g., config validation) would reduce duplication.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor exit-heavy functions to return `Result<!>` for easier testing and reuse.
related_specs:
  - ./main.rs.spec.md
  - ../core/src/auth.rs.spec.md
