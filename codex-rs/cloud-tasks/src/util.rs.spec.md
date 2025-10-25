## Overview
`util.rs` contains shared helpers for the Cloud Tasks CLI/TUI: user-agent customization, error logging, base URL normalization, JWT parsing, header construction, and task URL formatting.

## Detailed Behavior
- `set_user_agent_suffix` updates the global Codex user-agent suffix so backend requests identify the cloud client (`USER_AGENT_SUFFIX` mutex in `codex_core::default_client`).
- `append_error_log` appends timestamped messages to `error.log`, aiding offline debugging of backend interactions.
- `normalize_base_url` trims trailing slashes and ensures ChatGPT hosts end with `/backend-api`, matching the rules reused in the backend client.
- `extract_chatgpt_account_id` parses JWT payloads (`https://api.openai.com/auth.chatgpt_account_id`) to recover account ids when they are not explicitly provided.
- `build_chatgpt_headers` assembles `User-Agent`, `Authorization`, and `ChatGPT-Account-Id` headers using stored credentials (invokes `codex_login::AuthManager` and `extract_chatgpt_account_id` as fallback). Sets the user-agent suffix to `codex_cloud_tasks_tui` before fetching tokens.
- `task_url` constructs human-friendly URLs for opening tasks in a browser, handling various backend base URL suffixes (`/backend-api`, `/api/codex`, `/codex`).

## Broader Context
- Used throughout `lib.rs` during backend initialization, CLI submissions, and environment detection.
- Complements `codex-backend-client`’s normalization logic, ensuring both CLIs generate consistent API URLs and browser links.

## Technical Debt
- None; helper functions already handle the known base URL variants and gracefully fall back when auth tokens are missing.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./env_detect.rs.spec.md
