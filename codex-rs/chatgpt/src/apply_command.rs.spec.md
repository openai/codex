## Overview
`apply_command` wires the `codex chatgpt apply` subcommand into Codex configuration loading and diff application. It retrieves the latest hosted task diff and replays it against the local workspace.

## Detailed Behavior
- Defines `ApplyCommand`, a `clap::Parser` struct exposing `task_id` plus shared `CliConfigOverrides` so users can align CLI settings with local environments.
- `run_apply_command`:
  - Loads `codex_core::config::Config` using CLI overrides layered atop defaults.
  - Ensures the ChatGPT access token is initialized from Codex auth (`init_chatgpt_token_from_auth`).
  - Fetches the task data via `get_task`.
  - Hands control to `apply_diff_from_task`, optionally honoring a provided `cwd`.
- `apply_diff_from_task` extracts the current diff turn, searches for a PR output item, and forwards the embedded diff to `apply_diff`. Returns descriptive errors if the task lacks diff data.
- `apply_diff` builds a `codex_git_apply::ApplyGitRequest` and runs `apply_git_patch`, failing when git apply reports conflicts or skipped files. On success it prints a confirmation message.
- Helper functions favor signed integers and avoid unsigned types, aligning with repository style.

## Broader Context
- Depends on `get_task` and `chatgpt_token` modules plus `codex_core` configuration helpers to stay consistent with other CLI commands.
- Integrates with `codex-git-apply`, which shares specs under the Phase 3 tooling documentation.

## Technical Debt
- The CLI surfaces raw errors for missing diff turns; richer UX (e.g., listing available turns) could reduce user confusion when tasks are incomplete.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Improve diagnostics when no diff turn or PR output item exists (include task metadata or suggested next actions).
related_specs:
  - ../mod.spec.md
  - ./get_task.rs.spec.md
  - ./chatgpt_client.rs.spec.md
  - ./chatgpt_token.rs.spec.md
  - ../../git-apply/src/lib.rs.spec.md
