## Overview
`env_detect.rs` discovers Codex Cloud environments for the current workspace. It can either auto-select an environment based on Git remotes or fetch the full list from the backend, returning an `AutodetectSelection` with id/label.

## Detailed Behavior
- `CodeEnvironment` mirrors the backend environment payload (id, label, pinned flag, task count).
- `AutodetectSelection` is the simplified result returned to callers.
- `autodetect_environment_id` flow:
  1. Collect Git origins (`get_git_origins` tries `git config --get-regexp` then `git remote -v`).
  2. For each origin, extract `(owner, repo)` via `parse_owner_repo` (handles HTTPS/SSH, optional `.git`, GitHub only).
  3. Query `/environments/by-repo/<provider>/<owner>/<repo>` (WHAM or Codex API variant). Aggregate matches and pick a row via `pick_environment_row`.
  4. If no repo-specific match, fetch the full environment list (`/wham/environments` or `/api/codex/environments`), log payloads for diagnostics, and select via `pick_environment_row`.
  5. Returns `AutodetectSelection` or an error when no environments exist.
- `pick_environment_row` chooses an environment by label match, pinned flag, task count, or first entry.
- `get_json` is a helper for simple GET+JSON decode with detailed error messages.
- `get_git_origins` deduplicates URLs and gracefully handles command failures.
- `parse_owner_repo` recognizes `https://github.com/owner/repo(.git)` and `git@github.com:owner/repo(.git)` forms.
- `uniq` removes duplicates while preserving order.
- Extensive logging is routed through `util::append_error_log` for supportability.

## Broader Context
- Called by `lib.rs::resolve_environment_id` for explicit lookups and by background tasks to auto-detect defaults for the TUI.
- Works with `util::build_chatgpt_headers` to reuse authenticated headers when querying the backend.

## Technical Debt
- Currently GitHub-only; future providers would require extending `parse_owner_repo` and request URLs.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Support non-GitHub repository origins when Codex Cloud expands beyond GitHub.
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./util.rs.spec.md
