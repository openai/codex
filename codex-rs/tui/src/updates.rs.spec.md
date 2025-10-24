## Overview
`codex-tui::updates` checks for new Codex CLI releases and surfaces update prompts. It caches the latest version information, throttles network requests, and produces actionable update commands for the CLI to run after exit.

## Detailed Behavior
- Version caching:
  - `get_upgrade_version(config)` loads `version.json` under Codex home. If no cache exists or the last check is older than 20 hours, it spawns a background task (`tokio::spawn`) to refresh via GitHub’s latest release API.
  - Returns the cached latest version if it is newer than the running `CODEX_CLI_VERSION`.
  - `VersionInfo` stores `latest_version`, `last_checked_at`, and optionally a `dismissed_version`.
- HTTP refresh:
  - `check_for_update` fetches `https://api.github.com/repos/openai/codex/releases/latest`, strips the `rust-v` prefix, and writes updated JSON to `version.json`.
  - Uses `codex_core::default_client::create_client` for HTTP requests.
  - Ensures directory creation before writing.
- Popup logic:
  - `get_upgrade_version_for_popup` respects dismissed versions—returns `None` if the user previously dismissed the current latest version.
  - `dismiss_version` persists the dismissal to suppress future prompts for that version.
- Version comparisons:
  - `is_newer` and `parse_version` parse semantic versions (major.minor.patch) and compare them.
- Update actions:
  - `UpdateAction` enum enumerates package-manager-specific commands (npm, bun, brew).
  - `get_update_action` (non-debug builds) inspects environment/executable path to select an appropriate update command, returning `None` when self-managed.
  - `command_args` and `command_str` help the CLI print/run upgrade instructions.

## Broader Context
- `run_ratatui_app` invokes `get_upgrade_version` to prefetch updates and shows upgrade cells in release builds via `history_cell::UpdateAvailableHistoryCell`.
- CLI (`codex-rs/cli/src/main.rs`) runs chosen update commands after exit when `AppExitInfo::update_action` is set.
- Context can't yet be determined for non-GitHub release channels; the design is extensible if needed.

## Technical Debt
- Background tasks log errors via tracing but do not surface them to the UI; consider user-facing messages if update checks fail repeatedly.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Record metrics or user-visible indicators when update checks fail to avoid silent staleness.
related_specs:
  - ./app.rs.spec.md
  - ../cli/src/main.rs.spec.md
