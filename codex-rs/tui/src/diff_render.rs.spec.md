## Overview
`diff_render.rs` formats `FileChange` structures into terminal-friendly diff summaries for the TUI sidebar and dialogs. It produces both per-file renderables and aggregated views that highlight adds, deletes, renames, and moved content with consistent gutters, wrapping, and color styling. The module keeps rendering logic close to Ratatui widgets so diff snapshots remain readable even when lines wrap or when multiple files are bundled together.

## Detailed Behavior
- **Diff aggregation**: `DiffSummary` packages a `HashMap<PathBuf, FileChange>` plus the current working directory. Converting it into a `Renderable` builds a `ColumnRenderable` made of per-file chunks: a header row with relative paths and counters, a blank spacer, and the indented diff body via `InsetRenderable`.
- **FileChange rendering**: The `Renderable` impl on `FileChange` calls `render_change`, then surfaces the lines through a `Paragraph`. `desired_height` measures how many lines were produced so containers can allocate space without re-rendering.
- **Row preparation**: `collect_rows` snapshots add/delete counts using line lengths for new files or by parsing unified diffs via `calculate_add_remove_from_diff`. It also records rename targets and sorts rows by path to keep output deterministic.
- **Summary header**: `render_changes_block` emits an overall bullet summarizing how many files were touched and the total insert/delete counts. Single-file diffs skip repeating the filename in the body. Path strings are normalized by `display_path_for`, which prefers repo-relative paths when the file shares the caller’s Git root, otherwise falls back to `~`-relative display via `relativize_to_home`.
- **Diff bodies**: `render_change` handles the three `FileChange` variants. For `Add`/`Delete`, it counts the lines directly; for `Update`, it parses the unified diff (`diffy::Patch`) to compute the widest line number and to iterate hunks. A vertical ellipsis (`⋮`) separates hunks. Each diff line is formatted by `push_wrapped_diff_line`, which right-aligns the line number gutter, prefixes the content with `+`/`-`/` ` signs styled per diff type, and wraps long content while keeping continuation rows aligned.
- **Styling helpers**: Small helpers return shared `Style` objects for dim gutters, green inserts, red deletes, and plain context lines so we avoid recalculating modifiers.
- **Tests**: Snapshot tests render representative diffs into a `TestBackend` to pin layout for single additions, deletes, multi-file summaries, rename markers, and wrapping behavior. Text-based snapshots assert that wrapped lines keep signs on the first physical line and blank sign columns on continuations.

## Broader Context
- Higher-level renderers ([`render/mod.rs`](render/mod.rs.spec.md)) use these renderables when presenting staged changes, patch previews, or command summaries within the TUI timeline.
- `insert_history.rs` and similar history utilities display inline diffs while editing past turns, leveraging the same wrapping guarantees.
- Git metadata comes from `codex_core::git_info::get_git_repo_root` so summaries align with the core execution pipeline’s understanding of repositories.

## Technical Debt
- When `diffy::Patch::from_str` fails (malformed or truncated diff), the renderer silently falls back to zero counts and shows no diff body. Surfacing an explicit warning or placeholder block would help debugging.
- Wrapping logic splits on character indices but does not account for grapheme clusters or full-width plus combining marks, which could shift gutters for certain locales; extending tests to cover those cases would provide confidence.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Handle unparsable diffs by emitting a visible warning span so users know the diff could not be rendered.
    - Extend wrapping tests to cover multi-codepoint graphemes and ensure gutters stay aligned for emoji or combining marks.
related_specs:
  - ../mod.spec.md
  - render/mod.rs.spec.md
