## Overview
`core::tools::handlers::read_file` exposes a structured file-reading helper. It lets the model request either a simple line slice or an indentation-aware code block, enforcing absolute paths and size limits to prevent runaway reads.

## Detailed Behavior
- Accepts `ToolPayload::Function` with JSON arguments parsed into `ReadFileArgs`. Rejects other payload types.
- Validates that `offset`, `limit`, and (for indentation mode) `max_levels` and `max_lines` are greater than zero, and that `file_path` is absolute.
- `ReadMode::Slice` uses the `slice::read` helper to stream lines asynchronously with `tokio::io::BufReader`, trimming CRLF terminators, counting lines, and formatting each line via `format_line` (which numbers, truncates to `MAX_LINE_LENGTH`, expands tabs, etc.).
- `ReadMode::Indentation` uses `indentation::read_block` to collect a block anchored at an optional line, including surrounding context based on indentation depth, optional siblings, and header inclusion. It leverages `LineRecord` helpers to classify blank/comment lines and maintain indentation levels while respecting the global or per-block line limits.
- The handler joins the collected lines with newlines and returns them in `ToolOutput::Function { success: Some(true) }`.

## Broader Context
- This tool is gated behind an experimental feature flag in `tools/spec.rs`, allowing targeted rollout. It complements `list_dir` and `grep_files` by presenting readable snippets of large files without overwhelming context.
- Formatting utilities (tab expansion, comment detection) aim to produce IDE-like readability; adjustments here impact client rendering expectations.
- Context can't yet be determined for binary or non-UTF8 files; current logic assumes UTF‑8 input and will propagate decoding errors if encountered.

## Technical Debt
- TODOs note missing support for block comments in indentation mode; adding language-aware parsing would improve block extraction accuracy.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add block-comment awareness to indentation mode so comment blocks are handled consistently.
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
