Safe Sync & Merge Flow (No History Rewrites)

Goal
- Keep your local work intact while bringing in remote commits safely.
- Never rewrite history (no rebase/reset); use merge commits.
- On conflicts, get help from Codex TUI to resolve with minimal churn.

Quick Start
- One-time: ensure the script is executable: `chmod +x scripts/safe_sync_merge.sh`.
- Run: `just sync` (or `scripts/safe_sync_merge.sh`). From subdirectories, it detects the repo root and operates there.
- Defaults: merges from your tracking upstream (`@{u}`) if set; otherwise `origin/<current>`; else `origin/main` or `origin/master`.

What It Does
- Creates a safety backup branch: `backup/sync-<ts>-<branch>-<sha>`.
- Optionally stashes your uncommitted changes before the merge and restores them after.
- Fetches remotes with prune and merges upstream using `git merge --no-ff --no-edit`.
- On conflicts: optionally launches Codex TUI to assist resolution.
- After a successful merge: optionally runs `cargo fmt --check`, `clippy -D warnings`, and `cargo test --workspace`.

Usage
- `just sync` — run with defaults.
- `just sync -- --dry-run` — print planned actions without changing anything (note the `--` to pass args through `just`).
- `just sync --remote origin --upstream origin/main` — merge a specific upstream.
- `just sync --no-stash` — do not stash local changes (not recommended).
- `just sync --no-checks` — skip Rust checks after merging.
- `just sync --no-tui` — do not auto-launch TUI on conflicts.

Dry-run Sample Output
```
[safe-sync] Detected repo root: /path/to/your/repo
[safe-sync] Changing directory to repo root for consistent behavior
[safe-sync] Repository OK. Root: /path/to/your/repo. Current branch: main (abcdef123456)
[safe-sync] Upstream to merge: @{u}
[safe-sync] Dry run enabled: no changes will be made.
[safe-sync] Creating safety backup branch: backup/sync-YYYYmmdd-HHMMSS-main-abcdef123456
[dry-run] git branch backup/sync-YYYYmmdd-HHMMSS-main-abcdef123456
[safe-sync] Stashing local changes before merge
[dry-run] git stash push -u -m safe-sync:YYYYmmdd-HHMMSS
[safe-sync] Fetching from all remotes (with prune)
[dry-run] git fetch --all --prune
[safe-sync] Merging @{u} into main (no rebase, no ff)
[dry-run] git merge --no-ff --no-edit @\{u\}
[safe-sync] Merge completed successfully.
[safe-sync] Running post-merge checks in 'codex-rs' (fmt/clippy/tests)
[dry-run] cargo -C codex-rs fmt -- --check
[dry-run] cargo -C codex-rs clippy --workspace -- -D warnings
[dry-run] cargo -C codex-rs test --workspace
[safe-sync] Done. A safety backup was created at: backup/sync-YYYYmmdd-HHMMSS-main-abcdef123456
[safe-sync] Tip: if anything went wrong, you can inspect or reset to that branch.
```

Conflict Handling
- If a merge or stash pop conflicts, the script can open Codex TUI (`cargo run --bin codex -- tui`).
- Resolve conflicts in your editor or inside the TUI; then continue or re-run `just sync` to verify.

Recovery
- If anything feels off, you have a backup branch created before the merge.
- Inspect it or reset to it: `git checkout <your-branch> && git reset --hard <backup-branch>`.

Exit Codes & Remediation
- 2 — Merge conflicts detected
  - Fix: Resolve conflicts (use TUI or manual), then complete the merge; re-run `just sync` to verify.
- 3 — Stash pop conflicts after a successful merge
  - Fix: Resolve working tree conflicts, commit or re-stash as needed; re-run `just sync`.
- 4 — Formatting check failed (`cargo fmt -- --check`)
  - Fix: Run `cargo fmt` (or `just fmt`), stage, and retry.
- 5 — Clippy failed (`cargo clippy --workspace -- -D warnings`)
  - Fix: Address lints or run `cargo clippy --fix` cautiously, then retry.
 - 6 — Tests failed (`cargo test --workspace`)
   - Fix: Run `cargo test --workspace` locally, fix failures, and retry. For speed: `cargo test --workspace -- --quiet`.
 - N/A — No Rust workspace detected
   - Behavior: Script logs and skips checks if neither `Cargo.toml` nor `codex-rs/Cargo.toml` exists, exiting 0.
   - Marker: Emits `[safe-sync] SKIP_WORKSPACE` and sets `SAFE_SYNC_NO_WORKSPACE=1` so CI can detect and act.

Performance Tips
- Skip checks when needed: `just sync -- --no-checks`.
- Quieter/faster tests: `cargo test --workspace -- --quiet`.
- Deterministic deps in CI: `cargo test --locked` and `cargo clippy --locked` (if your lockfile is up to date).

CI Policy
- Resolve script paths via the canonical resolver and capture logs:
  - `bash codex-rs/scripts/resolve_safe_sync.sh --root "$GITHUB_WORKSPACE" --emit-gh-env >> "$GITHUB_ENV"`
  - `bash "$SAFE_SYNC" --dry-run --no-tui | tee safe_sync.log`
  - Fail CI if `[safe-sync] SKIP_WORKSPACE` appears (this repo expects a workspace).
- Run lightweight tests: `bash "$TEST_SCRIPT" all` and `bash "$TEST_SCRIPT" golden`.
- Real smoke: `cargo -C codex-rs fmt -- --check` on PRs to catch toolchain/config drift.
- Pin toolchain: Rust 1.89.0 with `rustfmt` (see `codex-rs/rust-toolchain.toml`).
- Locale pinning: CI/tests export `LC_ALL=C` and `LANG=C` to stabilize git hints; unset when debugging locale-specific issues.

Golden Snapshot (Drift Guard)
- A deterministic dry-run snapshot is stored at `codex-rs/tests/merge_flow_dry_run.snap`.
- CI runs `bash codex-rs/scripts/safe_sync_merge_test.sh golden` which:
  - clones the repo into a temp dir,
  - runs `safe_sync_merge.sh --dry-run`,
  - normalizes dynamic bits (paths, timestamps `YYYYmmdd-HHMMSS`, SHAs `abcdef123456`, upstream token `@{u}`),
  - compares byte-for-byte with the snapshot using `diff -u`.
- If the log strings change intentionally, update both the docs sample and the snapshot together.

Notes
- This flow avoids rebase/reset to protect your local history.
- If you prefer to keep your working tree dirty and still merge, use `--no-stash` (conflicts may increase).
 - Sample output omits dynamic values like timestamps and SHAs to reduce brittleness; real runs will show your actual values.

Path Resolution
- CI and local tooling should determine the script path using the shared helper `codex-rs/scripts/resolve_safe_sync.sh`.
- The resolver prefers `codex-rs/scripts` over root `scripts` to avoid drift and ensure consistent behavior across subdirs.
- The golden snapshot enforces an invariant of exactly 4 `YYYYmmdd-HHMMSS` placeholders; if format changes, update the snapshot and docs together.

Resolver CLI
- Usage:
  - `scripts/resolve_safe_sync.sh --root <repo_root>` (preferred) or positional `<repo_root>` for back-compat
  - `--emit-gh-env` prints GitHub Actions `KEY=VAL` pairs for `$GITHUB_ENV`
  - `--help` prints usage and exits 0
- Outputs (to stdout):
  - `SAFE_SYNC=<path>` • `TEST_SCRIPT=<path>` • `HAS_CODEX_RS=0|1` • `WORKSPACE_PRESENT=0|1`
- Exit codes:
  - `0` success
  - `2` scripts not found under `<root>/codex-rs/scripts` or `<root>/scripts`
  - `3` invalid or unreadable `--root`
