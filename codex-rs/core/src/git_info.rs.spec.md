## Overview
`core::git_info` collects repository metadata for telemetry, project trust, and rollout recordings. It locates the Git root, snapshots the current branch/commit, surfaces recent history, and computes diffs against the nearest remote ancestor—all without bundling libgit2.

## Detailed Behavior
- Discovery helpers:
  - `get_git_repo_root` walks upward from a directory until it finds a `.git` marker. `resolve_root_git_project_for_trust` expands to the common Git dir (handling worktrees) so trust checks use the canonical root.
  - `run_git_command_with_timeout` wraps `git` invocations in a 5 s timeout to avoid hangs on large repos; all higher-level helpers call through it.
- Metadata capture:
  - `collect_git_info` checks for a Git repo, then runs `rev-parse HEAD`, `rev-parse --abbrev-ref HEAD`, and `remote get-url origin` in parallel. Successful calls populate `GitInfo { commit_hash, branch, repository_url }`.
  - `recent_commits` fetches a bounded log (`git log -n N --pretty=format:%H%ct%s`) to drive pickers or telemetry cards, parsing commit SHAs, timestamps, and subjects.
  - `git_diff_to_remote` derives the closest remote SHA reachable from HEAD by:
    1. Determining branch ancestry (`branch_ancestry`), default branch (`get_default_branch`), and available remotes.
    2. Finding the first remote branch that contains HEAD via `branch_remote_and_distance`.
    3. Producing a unified diff (`diff_against_sha`) plus untracked file diffs (using `git diff --no-index` against `/dev/null`).
  - `GitDiffToRemote` stores the resulting SHA/diff for rollouts or trust prompts.
- Branch utilities:
  - `get_git_remotes`, `get_default_branch[_local]`, and `branch_ancestry` prioritize `origin` and fall back to local `main`/`master`.
  - `local_git_branches` lists branches with the default branch first; `current_branch_name` reports the currently checked-out branch.

## Broader Context
- Rollout recording (`rollout/recorder.rs`) calls `collect_git_info` to embed repository metadata into session logs. Trust checks use `resolve_root_git_project_for_trust` to decide which directories inherit approval.
- The CLI relies on `recent_commits` and `git_diff_to_remote` when presenting review summaries or recording provenance during tool actions.
- Context can't yet be determined for non-Git VCS; integrations would need parallel modules or feature flags to skip Git-specific prompts.

## Technical Debt
- Git commands rely on shelling out with ad-hoc parsing; migrating to a structured wrapper (or adding unit-tested parsers for each command) would reduce brittleness when Git output changes.
- `run_git_command_with_timeout` silently drops errors/timeouts; surfacing diagnostics would help users understand missing metadata without diving into logs.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace stringly parsing of Git commands with small, unit-tested parsers (or a helper crate) to make format changes less error-prone.
    - Surface timeout/errors from `run_git_command_with_timeout` to callers so UX can inform the user when Git metadata is unavailable.
related_specs:
  - ../mod.spec.md
  - ./project_doc.rs.spec.md
  - ./rollout/recorder.rs.spec.md
