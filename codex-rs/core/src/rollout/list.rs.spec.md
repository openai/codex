## Overview
`core::rollout::list` implements the read-path for rollout JSONL files. It paginates session summaries, loads entire conversations, and performs targeted lookups to support CLI/TUI inspection tooling.

## Detailed Behavior
- Data structures:
  - `ConversationsPage` wraps a page of `ConversationItem`s, pagination state, and scan metrics so callers know how much work was done.
  - `ConversationItem` captures paths, head/tail excerpts, and timestamps; the head retains metadata and earliest responses, the tail captures the most recent assistant turns.
  - `Cursor` encodes pagination state as `<timestamp>|<uuid>` with custom `Serialize`/`Deserialize` implementations to keep tokens opaque to clients.
- `get_conversations` walks the `sessions/` directory tree newest-first, respecting `MAX_SCAN_FILES`, and applies filters:
  - Directory traversal uses `collect_dirs_desc` / `collect_files` to read components asynchronously, sorting by parsed values.
  - Each file is summarized via `read_head_and_tail`, which parses the first N JSONL lines into structured JSON and samples the last N response records using backward chunk reads to avoid loading entire files.
  - Only sessions with a `SessionMeta` line and at least one user message event are returned; source filters drop non-interactive recordings.
  - Pagination resumes from a cursor by skipping files newer than the anchor timestamp/UUID.
- `get_conversation` reads an entire rollout file when detailed inspection is required.
- `find_conversation_path_by_id_str` reuses `codex_file_search` to locate rollouts by UUID, guarding against invalid identifiers and returning absolute paths.
- Helper routines (`parse_cursor`, `build_next_cursor`, `parse_timestamp_uuid_from_filename`) enforce consistent filename conventions (`rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`).

## Broader Context
- The listing API feeds `RolloutRecorder::list_conversations` so clients can surface recent sessions without reimplementing pagination.
- Tail sampling provides quick previews in UIs while keeping reads bounded, and pairs with the writer’s filtering logic from `policy.rs` to guarantee the JSON structure.
- Context can't yet be determined for archived rollouts; the traversal logic currently reads only the live `sessions/` tree.

## Technical Debt
- Directory traversal performs large sequential scans under a mutex-free design; on large fleets it may benefit from incremental indices or caching to reduce repeated IO.
- `read_head_and_tail` drops JSON parse errors silently; a higher-level error channel could flag corrupt rollouts before they reach users.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce caching or memoization for directory listings to reduce repeated scans when paginating large histories.
    - Surface partial-parse warnings (head/tail JSON failures) so operators can detect corrupt rollout files.
related_specs:
  - ./mod.rs.spec.md
  - ./recorder.rs.spec.md
  - ../git_info.rs.spec.md
