# PatchGate — Safety, Governance, Reproducibility

This doc explains the PatchGate flow implemented in `codex-rs/tui` and how to use it in pipelines.

- Safety (P0):
  - Applies diffs in an ephemeral git worktree created from `base_ref` and verifies lineage.
  - Strict `git apply` by default; 3-way fallback; rollback on failure; path traversal and `.git/**` blocked; per-task file lock.
- Governance (P1):
  - ChangeContract limits: per-file budgets (lines, hunks, bytes), max new files, allowed extensions.
  - Metadata guardrails: forbid symlinks, exec-bit/perms changes.
  - Secrets/minified scan (simple regex + heuristics), optional deny presets (e.g., `node_modules/**`).
  - Optional repo config: `.autopilot/config.toml` merges with the contract.
- Reproducibility (P2-01):
  - Commit trailers: `PRD-Ref`, `Contract-Hash`, `Diff-Hash`, `Task-Id`.
  - Artifacts: `.autopilot/rollouts/<TASK_ID>/<ts>/{envelope,contract,report}.json`.

See `docs/howto/patchgate.md` for integration, config, and CI examples.
