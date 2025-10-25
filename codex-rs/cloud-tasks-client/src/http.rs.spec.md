## Overview
`http.rs` implements the live `CloudBackend` using the REST endpoints exposed by Codex/ChatGPT. It layers on top of `codex-backend-client` for HTTP transport, derives richer summaries from the backend payloads, and delegates local patch application to `codex-git-apply`.

## Detailed Behavior
- `HttpClient` holds the normalized `base_url` and an underlying `backend::Client`. Builder-style `with_*` methods propagate bearer token, user agent, and ChatGPT account id to the backend client.
- Submodules (`api::Tasks`, `api::Attempts`, `api::Apply`) organize endpoint families:
  - **Tasks**
    - `list`: fetches the latest tasks (limit 20, `task_filter=current`), maps `TaskListItem` responses into `TaskSummary` with diff stats, review flags, and attempt counts.
    - `diff`: retrieves task details (and raw body) to extract unified diffs via `CodeTaskDetailsResponseExt::unified_diff`.
    - `messages`: returns assistant text output; falls back to worklog parsing when the structured fields are empty and surfaces error messages when tasks failed.
    - `task_text`: bundles user prompts, assistant messages, sibling turn ids, attempt placement, and status into a `TaskText`.
    - `create`: builds the JSON payload for new tasks (prompt message plus optional `CODEX_STARTING_DIFF`), attaches `metadata.best_of_n`, and posts via the backend client. Logs both successes and failures to `error.log`.
  - **Attempts**
    - `list`: wraps `list_sibling_turns`, converts loosely typed responses into `TurnAttempt` (sorting by placement/created_at), and extracts diff/messages from arbitrary JSON maps.
  - **Apply**
    - `run`: obtains a diff (from override or task details), verifies it’s a unified diff, and then calls `codex_git_apply::apply_git_patch` either in preflight or apply mode. Builds `ApplyOutcome` with skipped/conflict paths, logs detailed diagnostics (stdout/stderr tails, patch summary) on partial/error results, and normalizes messages for the CLI/TUI.
- Helper functions:
  - `details_path` chooses the right REST path based on `base_url`.
  - `extract_assistant_messages_from_body`, `turn_attempt_from_map`, `compare_attempts`, `extract_diff_from_turn`, `extract_assistant_messages_from_turn`, `attempt_status_from_str`, `parse_timestamp_value` interpret backend JSON structures that don’t map neatly onto generated models.
  - `map_task_list_item_to_summary` pulls environment labels and diff counts out of the “status display” metadata.
  - `is_unified_diff`, `tail`, `summarize_patch_for_logging` support apply diagnostics.
  - `append_error_log` mirrors `cloud-tasks` logging for consistent triage.

## Broader Context
- Used by the Cloud Tasks TUI/CLI whenever the `online` feature is enabled. It reuses backend-client functionality (rate limits, task listings, task details) and enriches responses with helper traits defined in `codex-backend-client::types`.
- Integration with `codex-git-apply` keeps patch application consistent with the git-aware tooling used elsewhere in Codex.

## Technical Debt
- Error logs may grow large; future work could rotate or truncate `error.log`. Additional backend endpoints (e.g., review tasks) will extend the helper modules.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce log rotation or truncation for `error.log` to prevent unbounded growth.
related_specs:
  - ../mod.spec.md
  - ./api.rs.spec.md
  - ./lib.rs.spec.md
