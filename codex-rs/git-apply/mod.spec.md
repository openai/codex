## Overview
`codex-git-apply` is a focused crate that applies unified diffs via `git apply`, with support for reversible patches, preflight checks, and comprehensive result parsing. It serves Codex components that need to stage or revert filesystem changes while reporting detailed outcomes.

## Detailed Behavior
- `ApplyGitRequest` captures the working directory, diff text, and flags controlling revert/preflight behavior.
- `apply_git_patch` orchestrates the workflow:
  - Resolves the git root, persists the diff to a temporary file, and optionally stages affected paths before a revert.
  - Assembles `git apply` arguments, honoring `CODEX_APPLY_GIT_CFG` for additional `git -c` settings.
  - If `preflight` is set, runs `git apply --check` and returns without modifying the working tree.
  - Otherwise executes `git apply --3way`, captures stdout/stderr, and parses result messages into applied/skipped/conflicted path lists.
- Helper utilities handle root resolution, command rendering for logs, diff path extraction, and staging logic so revert operations don’t clash with the index.
- `parse_git_apply_output` mirrors VS Code’s parser to extract path-level status from git’s output, capturing conflicts, skips, and applied files across stdout/stderr.
- Tests cover add/modify/delete flows, revert staging, preflight behavior, binary/partial failures, and other regression scenarios.

## Broader Context
- Used by Codex’s apply-patch tool handler and automation workflows to apply LLM-generated diffs safely with clear feedback.
- Complements the higher-level `apply-patch` crate by providing a git-backed fallback when patch parsing succeeds but native tooling needs to respect git metadata.

## Technical Debt
- None identified; functionality is intentionally scoped to git apply semantics.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
