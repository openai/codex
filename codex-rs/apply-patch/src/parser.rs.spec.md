## Overview
`parser.rs` implements the structured apply-patch grammar Codex expects from agents. It converts the pseudo-Lark format into `Hunk` structures, handling adds, deletes, updates, file moves, and trailing end-of-file markers with optional leniency for heredoc-wrapped inputs.

## Detailed Behavior
- Grammar constants define patch markers (`*** Begin Patch`, `*** Update File:` etc.) and change context tokens.
- `ParseError` enumerates invalid patches and hunks with clear messages and line numbers.
- `Hunk` variants (`AddFile`, `DeleteFile`, `UpdateFile`) store paths, contents, move targets, and chunked diffs; `resolve_path` joins relative paths against the provided cwd.
- `UpdateFileChunk` captures individual diff chunks with optional context, replacement lines, and EOF markers.
- `parse_patch` selects strict or lenient mode:
  - Strict mode enforces exact markers.
  - Lenient mode relaxes heredoc wrappers (strip `<<'EOF' ... EOF`), tolerates certain whitespace variations, and mirrors GPT-4.1 output quirks.
- Helper pipeline:
  - `parse_patch_text` checks boundaries, iterates hunks, and constructs `ApplyPatchArgs`.
  - `parse_one_hunk`/`parse_update_chunk`/`parse_move_to` parse file blocks and move directives.
  - Utility functions validate patch structure (e.g., `check_patch_boundaries_strict`, `expect_marker`, `parse_filename`) and guard against invalid paths or missing newline markers.
- Tests cover strict/lenient parsing, heredoc handling, error messaging, out-of-order chunks, invalid markers, and corner cases like empty files or trailing whitespace.

## Broader Context
- Consumed directly by `lib.rs` parsing functions (`parse_patch`, `maybe_parse_apply_patch_verified`) before safety checks or filesystem application.
- The lenient mode ensures compatibility with model output while still surfacing actionable errors for malformed patches.

## Technical Debt
- None noted; grammar and validation logic already balance strictness with necessary leniency.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
