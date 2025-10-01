# Forked Codex binary (project‑local)

Build and install
- ./scripts/build-codex-fork.sh
- This places `codex` at `tools/bin/codex`.

Use in this repo
- Add to PATH for this repo session:
  - `export PATH="$(pwd)/tools/bin:$PATH"`
- Or use direnv (`.envrc`):
  - `echo 'export PATH="$(pwd)/tools/bin:$PATH"' > .envrc && direnv allow`

Notes
- This binary is functionally compatible with upstream. Prehook is disabled by default and opt‑in via flags/env.
- To revert to upstream, remove the PATH override or invoke the upstream `codex` by absolute path.
