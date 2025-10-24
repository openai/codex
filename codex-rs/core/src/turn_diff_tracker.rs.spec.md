## Overview
`core::turn_diff_tracker` accumulates file diffs across a turn so Codex can present a unified patch to users. It snapshots baseline file content, tracks renames, computes Git-style unified diffs, and falls back to binary indicators when necessary. The tracker is used by task runners to summarize changes before emitting final responses.

## Detailed Behavior
- Maintains parallel maps:
  - `external_to_temp_name`: maps current filesystem paths to stable internal UUIDs.
  - `baseline_file_info`: stores the initial content, file mode, and blob OID captured when a file is first touched.
  - `temp_name_to_current_path`: tracks the latest external path for each internal UUID, enabling rename detection.
  - `git_root_cache`: memoizes discovered Git repository roots to avoid repeated directory walks.
- `on_patch_begin` processes apply-patch deltas:
  - Assigns internal IDs to unseen paths and snapshots baseline content (or zeros for new files).
  - Records file modes and Git blob IDs (preferring `git hash-object` output when available).
  - Updates mappings for files with `move_path`, ensuring renames are reflected in subsequent diffs.
- `get_unified_diff` sorts tracked files by repo-relative path, concatenates per-file diffs from `get_file_diff`, adds trailing newlines, and returns `None` when no changes remain.
- `get_file_diff` compares baseline bytes with current disk content:
  - Computes display paths relative to the Git root (fallback to absolute) and normalizes path separators.
  - Determines add/delete/rename scenarios and prints Git headers (`diff --git`, `index`, mode changes).
  - Uses `similar::TextDiff` to generate unified diffs for text files; otherwise emits a binary file notice.
  - For symlinks, hashes the target path to match Git semantics.
- Helper methods:
  - `find_git_root_cached` walks up directories to locate `.git`, caching positive hits.
  - `git_blob_oid_for_path` runs `git hash-object` to match the repository’s object IDs.
  - `blob_bytes` reads file bytes or symlink targets; on non-Unix platforms symlink content is unsupported.
  - `file_mode_for_path` infers executable/symlink modes (Unix), defaulting to regular files elsewhere.
  - `git_blob_sha1_hex_bytes` reproduces Git blob hashing for fallback cases.
- Tests use `tempfile` to validate additions, updates, deletions, and renames, normalizing diff headers for determinism.

## Broader Context
- The tracker ensures Codex emits Git-compatible diffs without shelling out to `git diff` per change. This is crucial for environments where Git isn’t available or working directories aren’t repositories.
- Rollout logging and review output rely on these diffs to provide actionable patches; any divergence from Git’s formatting can break downstream tooling.
- Context can't yet be determined for large binary files; current behavior emits “Binary files differ,” which may suffice until richer binary diff presentation is required.

## Technical Debt
- The implementation shells out to `git` to compute blob hashes when possible; environments without Git fall back to local hashing. Detecting and warning about missing Git earlier could improve diagnostics.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Add structured logging when `git hash-object` fails so users understand why fallback hashes are used.
related_specs:
  - ./tasks/mod.spec.md
  - ./codex.rs.spec.md
