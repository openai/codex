Resolver: scripts/resolve_safe_sync.sh

Purpose
- Provide a single, canonical way to discover the safe sync/merge script paths across layouts.
- Prefer `codex-rs/scripts` over repository root `scripts` to avoid drift.

CLI
- Preferred:
  - `scripts/resolve_safe_sync.sh --root <repo_root>`
- Backward‑compatible (deprecated):
  - `scripts/resolve_safe_sync.sh <repo_root>`
- CI helper:
  - `--emit-gh-env` prints `KEY=VAL` lines suitable for `$GITHUB_ENV` (SAFE_SYNC, TEST_SCRIPT, HAS_CODEX_RS, WORKSPACE_PRESENT)
- Help:
  - `--help` prints usage and exit codes; includes deprecation notice for positional `<repo_root>`

Exit Codes
- 0 — success
- 2 — not found under `<root>/codex-rs/scripts` nor `<root>/scripts`
- 3 — invalid or unreadable `--root`

Outputs
- `SAFE_SYNC=<path>` — absolute path to `safe_sync_merge.sh`
- `TEST_SCRIPT=<path>` — absolute path to `safe_sync_merge_test.sh`
- `HAS_CODEX_RS=0|1` — whether `<root>/codex-rs/scripts` was selected
- `WORKSPACE_PRESENT=0|1` — whether a workspace file was detected (`Cargo.toml` at root or `codex-rs/Cargo.toml`)

Examples
- Resolve for CI and run dry‑run:
  - `bash scripts/resolve_safe_sync.sh --root "$GITHUB_WORKSPACE" --emit-gh-env >> "$GITHUB_ENV"`
  - `bash "$SAFE_SYNC" --dry-run --no-tui | tee safe_sync.log`

Notes
- Tests/CI pin locale (`LC_ALL=C`, `LANG=C`) to stabilize git hint lines.
- Positional `<repo_root>` remains for back‑compat; prefer `--root` in new tooling.
- If paths change, update this doc, the resolver, and the tests together to keep the contract in sync.
 - Golden help snapshot lives at `docs/golden/resolver_help.txt`. To update after intentional changes:
   - Preferred: `just update-resolver-golden`
   - Exact normalization (matching tests): `LC_ALL=C LANG=C bash scripts/resolve_safe_sync.sh --help | tr -d '\r' | awk 'NF{print $0}' ORS='\n' > docs/golden/resolver_help.txt`
   - Keep wording stable to avoid needless churn. See `docs/CONTRIBUTING.md` for details.
