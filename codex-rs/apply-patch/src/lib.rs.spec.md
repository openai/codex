## Overview
`lib.rs` is the core of the apply-patch engine. It parses structured patches emitted by Codex models, validates and classifies the requested changes, and applies hunks to the filesystem while reporting clear diagnostics. It also exposes helper APIs that other crates use to detect `apply_patch` invocations, interpret proposed changes, and run the standalone tool.

## Detailed Behavior
- Patch detection & parsing:
  - `maybe_parse_apply_patch` inspects argv arrays for direct `apply_patch`/`applypatch` calls or `bash -lc` heredoc invocations, returning `MaybeApplyPatch` results that differentiate between body payloads, shell parsing errors, and non-matches.
  - `maybe_parse_apply_patch_verified` adds safeguards: rejects implicit raw patch bodies, resolves working directories (including `cd` heredoc prefixes), loads existing file contents for delete/update hunks, and materializes an `ApplyPatchAction` (absolute paths mapped to `ApplyPatchFileChange` variants).
  - Tree-sitter Bash is used via `extract_apply_patch_from_bash` (with cached `Query`) to conservatively match supported heredoc forms, avoiding false positives.
  - Public types (`ApplyPatchArgs`, `ApplyPatchAction`, `ApplyPatchFileChange`, `MaybeApplyPatchVerified`) describe parsed state for downstream tooling (e.g., safety checks, previews).
- Application pipeline:
  - `apply_patch` parses the patch snippet, writes friendly errors to stderr on parse failures, and delegates to `apply_hunks`.
  - `apply_hunks` calls `apply_hunks_to_files`, then prints a summary (`print_summary`) of added/modified/deleted files to stdout; errors are propagated via `ApplyPatchError` variants (wrapping IO/compute errors).
  - `apply_hunks_to_files` iterates hunks, creating directories as needed, writing new contents, handling file moves, and returning an `AffectedPaths` summary.
  - `derive_new_contents_from_chunks`, `compute_replacements`, and `apply_replacements` manage diff application logic, using `seek_sequence` to locate contexts and `similar::TextDiff` for move detection (populating `Hunk::UpdateFile::move_path` details).
- Constants:
  - `APPLY_PATCH_TOOL_INSTRUCTIONS` embeds operator guidance for LLM prompts.
  - `APPLY_PATCH_COMMANDS` enumerates accepted command names.
- Error handling:
  - `ApplyPatchError` wraps parse, IO, implicit-invocation, and computation failures; conversions from `std::io::Error` attach helpful context.
  - `ExtractHeredocError` captures tree-sitter load failures, UTF-8 issues, and unmatched heredoc patterns.
- Tests span parsing, heredoc extraction, diff application (including add/delete/update/move cases), summary output, and failure scenarios (missing files, conflicting changes, escaped paths).

## Broader Context
- Consumed by Codex’s tool-orchestration layer (`core/src/tools/handlers/apply_patch.rs.spec.md`) to vet patches before execution, calculate safety checks, and provide user-facing previews.
- Works alongside `codex-git-apply` when git-level validation or staging is necessary; `ApplyPatchAction` feeds safety modules that examine path coverage.
- Tree-sitter and regex dependencies highlight the need to keep parsing logic conservative as agent prompts evolve.

## Technical Debt
- None flagged; lenient parsing, context searches, and error mapping encapsulate current model and filesystem edge cases.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./parser.rs.spec.md
  - ./seek_sequence.rs.spec.md
  - ./standalone_executable.rs.spec.md
