## Overview
`message_history` persists every Codex conversation turn to an append-only JSONL log (`~/.codex/history.jsonl`). It provides async helpers for writers, metadata queries for readers, and a Unix-specific lookup API so UIs can paginate through historical messages.

## Detailed Behavior
- `HistoryEntry` captures the serialized schema (`session_id`, Unix timestamp, message text). `history_filepath` builds the per-user path inside the configured Codex home directory.
- `append_entry`:
  - Respects the user’s history persistence setting (`HistoryPersistence::SaveAll` vs. `None`).
  - Creates the target directory, computes the current timestamp, serializes the entry, and writes the full JSON line in append-only mode with `O_APPEND`.
  - Uses `ensure_owner_only_permissions` to enforce `0600` permissions on Unix, then acquires an advisory exclusive lock (`try_lock`) inside a blocking task, retrying up to `MAX_RETRIES` with `RETRY_SLEEP` backoff to avoid interleaving writers.
  - Contains a TODO to scan outgoing text for sensitive patterns before writing.
- `history_metadata` returns the file identifier (inode on Unix) and the number of entries by counting newlines asynchronously.
- `lookup` (Unix):
  - Verifies the file’s inode matches the provided `log_id`, then reads line-by-line under a shared advisory lock to fetch the requested offset.
  - Logs and returns `None` if the file changes, parsing fails, or locks cannot be acquired.
  - Falls back to a no-op stub on non-Unix platforms.
- `ensure_owner_only_permissions` enforces private permissions asynchronously; the non-Unix variant is currently a no-op.

## Broader Context
- Used by the CLI/TUI history viewers (`../../tui/src/session_log.rs.spec.md`) and potential future analytics tooling to provide persistent transcripts.
- Dependent on configuration plumbing (`./config.rs.spec.md`, `./config_types.rs.spec.md`) for enable/disable switches and home-directory resolution.
- Integrates with `codex_protocol::ConversationId` to associate log entries with durable conversation identifiers shared across services.

## Technical Debt
- Needs a redact/validation pass before persisting history entries to protect sensitive information (`TODO: check text for sensitive patterns`).

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add sensitive-data filtering before writing history entries.
related_specs:
  - ./config.rs.spec.md
  - ./config_types.rs.spec.md
  - ../../tui/src/session_log.rs.spec.md
