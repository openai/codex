## Overview
Async helper that mirrors the CLI’s Git diff routine. It returns whether the current working directory is within a Git repository and, if so, concatenates tracked changes plus synthetic diffs for untracked files using `git diff --no-index`.

## Detailed Behavior
- `get_git_diff` first calls `inside_git_repo`; if false it returns `(false, String::new())` immediately.
- When inside a repo it runs two commands in parallel:
  - `git diff --color` to capture tracked modifications (`run_git_capture_diff` treats exit code 1 as success because Git uses it to signal “diff present”).
  - `git ls-files --others --exclude-standard` to list untracked files (`run_git_capture_stdout` expects exit status 0).
- For each untracked path it spawns a Tokio task that executes `git diff --color --no-index -- /dev/null <file>` (or `NUL` on Windows) so the UI can preview additions. Results are accumulated into `untracked_diff`. `io::ErrorKind::NotFound` is ignored to accommodate race conditions where files vanish before diffing.
- The final return is `(true, format!("{tracked}{untracked}"))`.
- Helper functions centralize command invocation, capturing stdout as UTF-8 and raising `io::Error` on other exit statuses. `inside_git_repo` runs `git rev-parse --is-inside-work-tree` and gracefully reports `false` when Git is missing.

## Broader Context
- The TUI uses this diff output before generating change previews and for status overlays. It keeps behavior aligned with the TypeScript CLI so both surfaces show identical git summaries.
- Downstream renderers (`diff_render.rs`) consume the resulting `String` via core diff parsing logic before painting summaries.

## Technical Debt
- Uses Tokio’s `JoinSet` with default task limits; if many untracked files exist, we launch one task per file with no throttling, which can inflate process count and disk pressure.
- `git` path and arguments are hard-coded. Detecting Git absence returns `false`, but surfacing a user-facing notice might improve diagnostics.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Throttle or batch untracked file diff tasks to avoid overwhelming systems with large numbers of new files.
    - Emit a user-visible warning when Git commands fail for reasons other than “not a repo” so errors aren’t silently swallowed.
related_specs:
  - mod.spec.md
  - diff_render.rs.spec.md
