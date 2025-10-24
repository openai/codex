## Overview
`core::project_doc` assembles the project-level instructions that supplement Codex’s built-in guidance. It discovers `AGENTS.md` (and overrides) along the path from the repository root to the current working directory, enforces a byte budget, and merges the result with any `Config::user_instructions` before requests are sent to the model.

## Detailed Behavior
- `get_user_instructions` orchestrates the merge. It awaits `read_project_docs`, concatenates the project doc with existing `user_instructions` using `--- project-doc ---` as a separator, and degrades gracefully: failures are logged and the original instructions are returned unchanged.
- `read_project_docs` exits early when `project_doc_max_bytes` is zero or no files are found. Otherwise, it streams each discovered file through a `tokio::io::BufReader`, truncating content to the remaining byte budget, warning when truncation occurs, discarding empty segments, and joining sections with a blank line.
- `discover_project_doc_paths` normalizes `Config::cwd`, walks upward until it finds a `.git` marker, and then lists the directories from the Git root down to `cwd`. For each directory it picks the first matching filename from the candidate list (favoring the local override, then the default, then configured fallbacks) and accepts both regular files and symlinks.
- `candidate_filenames` builds the ordered search list starting with `AGENTS.override.md` and `AGENTS.md`, then appends unique, non-empty entries from `project_doc_fallback_filenames`.
- Tests cover missing files, byte-limit truncation, Git-root discovery, override precedence, fallback usage, concatenation ordering, and integration with pre-existing `user_instructions`.

## Broader Context
- `codex.rs` calls `get_user_instructions` before launching a turn so the conversation manager and client layers work with a single instruction string.
- The module relies on `Config` for runtime knobs: working directory, project-doc byte budget, configured fallbacks, and any preloaded system instructions.
- Project docs give workspace maintainers a way to codify coding standards alongside `Config::user_instructions`; downstream specs describing client prompt assembly should reference this merge behavior to explain why instructions already include project metadata.

## Technical Debt
- `discover_project_doc_paths` uses blocking filesystem metadata checks inside async flows; switching to `tokio::fs` would avoid potential stalls in slow filesystems while keeping behavior identical.
- Truncation drops the remainder silently after logging; providing a marker or surfacing the overflow to the caller could help teams verify that critical doc content is not clipped by the byte budget.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Replace blocking metadata calls in `discover_project_doc_paths` with async equivalents to prevent IO pauses on networked filesystems.
    - Consider returning truncation metadata so callers can alert users when project docs exceed `project_doc_max_bytes`.
related_specs:
  - ../mod.spec.md
  - ./codex.rs.spec.md
  - ./config.rs.spec.md
