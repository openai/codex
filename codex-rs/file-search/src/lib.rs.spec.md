## Overview
`lib.rs` implements the core fuzzy filename search engine. It coordinates CLI parameters, traverses the filesystem in parallel, and produces scored `FileMatch` results that callers can render or serialize however they like.

## Detailed Behavior
- `Cli` (re-exported) captures all command-line options so the binary and downstream crates share a single parser.
- `FileMatch` and `FileSearchResults` describe the structured output, including optional highlight indices when `compute_indices` is enabled.
- `Reporter` abstracts result delivery; `run_main` consumes a `Cli` and `Reporter`, resolves the search directory, and:
  - Falls back to an `ls -al` listing (Unix) or `cmd /c` (Windows) when no pattern is provided, after logging a warning.
  - Invokes `run` to perform the actual search, then streams matches and truncation warnings to the provided reporter.
- `run` builds the fuzzy pattern via `nucleo_matcher`, derives worker counts from `--threads`, and spins up `ignore::WalkBuilder` for parallel traversal. Key behaviors:
  - Uses per-worker `BestMatchesList` instances stored in `UnsafeCell`s, allowing worker closures to update their local heaps without locking.
  - Applies `--exclude` glob overrides, allowing callers to prune directories or files.
  - Periodically checks `cancel_flag` to exit early and returns empty results if cancellation triggers mid-run.
  - After traversal, merges worker heaps into a global `BinaryHeap`, sorts matches (`sort_matches`/`cmp_by_score_desc_then_path_asc`), optionally computes highlight indices, and returns the final `FileSearchResults`.
- Helper utilities include `BestMatchesList::insert` (scoring individual paths), `create_worker_count` (accounts for ignore’s thread semantics), and `create_pattern` (configures smart case/normalization).
- Tests verify matcher behavior, tie-breaking, and ensure `Pattern::score` semantics remain consistent.

## Broader Context
- The MCP server (`../mcp-server/src/fuzzy_file_search.rs.spec.md`) wraps `run` to power fuzzy search inside Codex integrations.
- Complements CLI usage (`main.rs`) by exposing a cancellation-aware API for other async components.

## Technical Debt
- None noted; concurrency and traversal edge cases are handled in the current design.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./cli.rs.spec.md
  - ./main.rs.spec.md
