# PatchGate — How‑To

This guide shows how to integrate PatchGate for PRD-driven diffs and minimal churn.

## Worktree Policy
- The gate applies diffs in an ephemeral worktree created from `base_ref`, on branch `autopilot/<TASK_ID>`.
- Lineage is verified; on failures, the worktree is force-removed; no dirty state leaks.

## ChangeContract (governance)
- Core fields: `allowed_paths`, `deny_paths`, `max_files_changed`, `max_lines_added`, `max_lines_removed`, `allow_renames`, `allow_deletes`, `forbid_binary`, `require_tests`, `commit_prefix`, `require_signoff`.
- P1 fields:
  - Per-file budgets: `max_new_files`, `max_bytes_per_file`, `max_lines_added_per_file`, `max_hunks_per_file`.
  - Allowed extensions: `allowed_extensions` (empty = allow all).
  - Metadata guardrails: `forbid_symlinks`, `forbid_permissions_changes`, `forbid_exec_mode_changes`.
  - Secrets/minified: `forbid_secrets`, `forbid_minified`.
  - Deny presets: `deny_presets` (e.g., `node_modules`, `dist`, `vendor`).

## Repo Config
Optional `.autopilot/config.toml` (merged into contract):

```
deny_presets = ["node_modules", "dist"]
forbid_secrets = true
forbid_minified = true
```

Config precedence (recommended): env > repo config > defaults.

## Commit Trailers & Artifacts
- Trailers appended to the commit:
  - `PRD-Ref: <sha256(PRD.md)>`
  - `Contract-Hash: <sha256(serde_json(contract))>`
  - `Diff-Hash: <sha256(diff)>`
  - `Task-Id: <id>`
- Artifacts persisted under `.autopilot/rollouts/<TASK_ID>/<ts>/`:
  - `envelope.json` • `contract.json` • `report.json`

## CI (GitHub Actions) — Example
Save as `.github/workflows/patchgate.yml`:

```yaml
name: PatchGate CI
on:
  pull_request:
  push:
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.89.0
          components: clippy,rustfmt
      - name: Clippy (workspace)
        run: cargo clippy --workspace -- -D warnings
      - name: Test (1)
        run: cargo test --workspace -- --nocapture
      - name: Test (2)
        run: cargo test --workspace -- --nocapture
      - name: Upload PatchGate artifacts (best-effort)
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: autopilot-rollouts
          path: .autopilot/rollouts/**
          if-no-files-found: ignore
```

## Make targets — Example
You can expose convenience targets for local runs. Example `Makefile` snippet for an integrating repo:

```
.PHONY: patchgate-check patchgate-apply

# Provide FILE pointing to a Builder-produced envelope.txt
patchgate-check:
	@echo "Running PatchGate dry-run (check only)"
	@cargo test -p codex-tui --test ephemeral_worktree_trailers_and_artifacts -- --nocapture

patchgate-apply:
	@echo "Running PatchGate apply in a throwaway repo (demo)"
	@cargo test -p codex-tui --test patchgate_smoke -- --nocapture
```

Notes:
- Replace the examples with a CLI that invokes `verify_and_apply_patch` in your environment, feeding a real Diff Envelope from the Builder and a ChangeContract derived from PRD.
- On Windows, prefer adding `git worktree prune` after runs; ensure no processes hold open files under worktrees.

## TUI integration
The TUI calls the gate via `run_patch_gate_for_builder_output`, rendering a concise badge with status, stats, and top violations.
If the Builder output is invalid (not a single envelope), TUI shows:

hint: output must be a single Diff Envelope (see docs)

Metrics badge line (example):

```
rejections_total{enforcement}=0 • patchgate_apply_seconds=0.12 • ci_runs_total{pre}=0 post=0
```

Valid examples (envelope body):
- Unified diffs with content hunks
- Delete-only or mode-only diffs (headers present; no hunks necessary)
- Rename/copy-only diffs (e.g., `similarity index` with `rename from/rename to` or `copy from/copy to`)

Invalid examples:
- Extra prose before/after the `<diff_envelope>…</diff_envelope>` block

Example TUI message on invalid output (fenced or prose):

```
🧩 PatchGate: REJECTED
invalid Builder output: markdown fences detected; envelope must be plain text
hint: output must be a single Diff Envelope (see docs)
```
