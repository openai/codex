## Overview
`codex-apply-patch` provides Codex’s native patch parser and applier. It parses the structured patch format we ask models to emit, validates context, applies file system changes (including moves), and offers a standalone binary for local testing.

## Detailed Behavior
- `lib.rs` is the central API:
  - Exposes `parse_patch`, `maybe_parse_apply_patch_verified`, `apply_patch`, and related types (`ApplyPatchAction`, `ApplyPatchFileChange`, `ApplyPatchError`).
  - Parses heredoc-based invocations with Tree-sitter Bash, resolves working directories, and computes unified diffs/outgoing file contents.
  - Applies hunks to disk, creating/removing/moving files as needed and printing a summary of affected paths.
  - Defines `APPLY_PATCH_TOOL_INSTRUCTIONS`, the prompt injected into model tooling.
- `parser.rs` implements the structured patch grammar, supporting lenient parsing for heredoc-wrapped arguments (e.g., GPT-4.1 quirks) while preserving strict validation for malformed hunks.
- `seek_sequence.rs` locates patch contexts within existing files, tolerating whitespace and certain Unicode punctuation differences to mirror `git apply`’s resilience.
- `standalone_executable.rs` provides the `main`/`run_main` entrypoint so the crate can be built as the `apply_patch` command-line tool, reading patches from argv or stdin and returning sensible exit codes.
- `main.rs` simply delegates to the library main for the published binary.

## Broader Context
- The crate underpins Codex’s plan/tool execution pipeline, giving sandboxed agents a precise, auditable way to mutate files without shelling out to `patch`.
- Works in tandem with `codex-git-apply` and `codex-git-tooling` when git-aware reconciliation or rollback is required.
- Tree-sitter Bash dependency ensures heredoc parsing stays robust as LLM output evolves.

## Technical Debt
- None immediately; lenient parsing accommodates current model behaviors, and tree-sitter queries remain conservative.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/parser.rs.spec.md
  - ./src/seek_sequence.rs.spec.md
  - ./src/standalone_executable.rs.spec.md
  - ./src/main.rs.spec.md
