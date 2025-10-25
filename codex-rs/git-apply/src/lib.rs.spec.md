## Overview
`lib.rs` drives the git-backed patch application engine. It exposes request/result types, orchestrates git invocations for both dry-run and live apply modes, and parses git’s output to report which paths were applied, skipped, or conflicted.

## Detailed Behavior
- `ApplyGitRequest` fields:
  - `cwd` repo working directory.
  - `diff` unified diff text to apply.
  - `revert` toggles `git apply -R` to reverse a patch.
  - `preflight` triggers a `--check` dry run without touching the workspace.
- `ApplyGitResult` records exit code, categorized paths (`applied_paths`, `skipped_paths`, `conflicted_paths`), stdout/stderr, and a human-readable command string for logging.
- `apply_git_patch` workflow:
  1. Resolves the repository root via `git rev-parse --show-toplevel`.
  2. Writes the diff to a temporary file and keeps the tempdir alive for command execution.
  3. When reverting without preflight, stages existing paths listed in the diff so git apply sees matching index state.
  4. Builds the git command (`git apply --3way`, optionally `-R`) and prepends any `git -c` options parsed from `CODEX_APPLY_GIT_CFG`.
  5. For preflight mode, runs `git apply --check`, parses output, and returns immediately.
  6. Otherwise executes `git apply`, parses output, deduplicates sorted path lists, and returns the assembled `ApplyGitResult`.
- Supporting helpers:
  - `resolve_git_root`, `write_temp_patch`, `run_git`, `render_command_for_log`, and `quote_shell` manage process execution and log-friendly strings.
  - `extract_paths_from_patch` identifies impacted files in the diff to stage for reverts.
  - `stage_paths` stages existing files best-effort; non-zero exit status is ignored to avoid failing the entire operation.
  - `parse_git_apply_output` uses case-insensitive regexes to classify git messages, tracking applied/skip/conflict states and handling three-way merge commentary.
- Tests validate positive and negative scenarios, preflight vs. live applications, revert behavior, and environment isolation.

## Broader Context
- Invoked by Codex’s apply-patch workflows when git integration is required, feeding results back to tool handlers that display per-file status to users.
- Complements snapshot tooling (`codex-git-tooling`) by providing the apply path while ghost commits cover rollback.

## Technical Debt
- None noted; open TODOs from the VS Code port have already been addressed or documented in tests.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ../../git-tooling/mod.spec.md
