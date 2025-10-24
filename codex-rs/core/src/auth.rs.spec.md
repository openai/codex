## Overview
`core::auth` manages authentication for Codex. It loads credentials from `auth.json`, supports API-key and ChatGPT (OAuth) modes, refreshes tokens, and enforces login restrictions configured in `Config`. The module also provides helpers for reading environment variables, writing `auth.json`, and logging out.

## Detailed Behavior
- `CodexAuth` represents the active authentication context:
  - Fields include mode (`AuthMode::ApiKey` or `AuthMode::ChatGPT`), cached tokens (`Arc<Mutex<Option<AuthDotJson>>>`), `auth.json` path, and a shared `reqwest::Client`.
  - `from_codex_home`/`load_auth` read `auth.json`, preferring API keys when present; `CODEX_API_KEY` allows overriding credentials for automation/tests.
  - `get_token`, `get_token_data`, and `refresh_token` retrieve/refresh access tokens (refreshing when `last_refresh` is older than 28 days). Token refresh uses `try_refresh_token`, then updates `auth.json` via `update_tokens`.
  - Accessors expose account metadata (`get_account_id`, `get_account_email`, `get_plan_type`), used for workspace restrictions and telemetry.
- Environment helpers (`read_openai_api_key_from_env`, `read_codex_api_key_from_env`) supply credentials from env vars when available. `login_with_api_key` and `logout` manipulate `auth.json` directly.
- `enforce_login_restrictions`:
  - Validates `ForcedLoginMethod` (ensuring configuration requires the intended mode) and, for ChatGPT logins, verifies workspace ID matches `forced_chatgpt_workspace_id`.
  - On violations or token load errors, logs out by deleting `auth.json` and returns an error message to surface to the user.
- JSON handling:
  - `try_read_auth_json`/`write_auth_json` read/write `auth.json` with 0600 permissions on Unix.
  - `update_tokens` updates the JWT contents (using `token_data::parse_id_token`) and refresh tokens, bumping `last_refresh`.
- Tests include serialized scenarios (via `serial_test`) to avoid filesystem conflicts.

## Broader Context
- `Config::load_from_base_config_with_overrides` consumes `CodexAuth` to determine provider-specific headers; tool runtimes rely on `CodexAuth::get_token` for bearer auth.
- Token parsing leverages `token_data.rs`; network calls use `default_client::create_client` to ensure consistent headers/user agent.

## Technical Debt
- None explicit; global client(s) and `LazyLock` patterns are intentional for reuse. Token refresh behaviour may evolve alongside new providers.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./token_data.rs.spec.md
  - ./default_client.rs.spec.md
  - ./config.rs.spec.md
