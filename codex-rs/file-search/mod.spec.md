## Overview
`codex-file-search` provides Codex’s fuzzy filename search engine and the accompanying CLI. The crate wraps `ignore`’s parallel directory walker and `nucleo_matcher` to deliver scored, cancellable matches across large workspaces, and exposes a reporter-driven API that the binary and other crates can reuse.

## Detailed Behavior
- `src/lib.rs` hosts the search engine:
  - Exposes the `Cli` arguments, `run_main` async entrypoint, lower-level `run` worker, and helper types (`FileMatch`, `FileSearchResults`, `Reporter`).
  - Uses `WalkBuilder` to traverse the tree in parallel, honors `--threads` and exclude globs, and keeps per-worker heaps of top matches merged at the end.
  - Supports cancellation via an `Arc<AtomicBool>` and optional highlight indices through `nucleo_matcher::Pattern::indices`.
- `src/cli.rs` defines the Clap interface, encapsulating options such as JSON output, match limit, thread count, exclude patterns, and index computation.
- `src/main.rs` wires the CLI into a `StdioReporter` that prints plain paths, ANSI-highlighted matches, or JSON objects depending on flags, and pipes truncation warnings or `ls` fallbacks when no pattern is provided.

## Broader Context
- Used by the MCP server (`../mcp-server/src/fuzzy_file_search.rs.spec.md`) and other services that rely on fuzzy file selection; the CLI doubles as a debugging and support tool.
- Complements Codex’s richer search and apply-patch tooling by providing a fast, standalone filename matcher.

## Technical Debt
- None currently identified; behavior and concurrency limits are documented in the source comments.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/cli.rs.spec.md
  - ./src/main.rs.spec.md
