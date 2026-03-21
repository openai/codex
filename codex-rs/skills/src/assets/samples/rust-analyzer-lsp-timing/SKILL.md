---
name: rust-analyzer-lsp-timing
description: Benchmark end-to-end Rust edit feedback loops using a long-lived rust-analyzer process (LSP) and record diagnostic latency.
---

# Rust Analyzer LSP Timing

Use this skill when you want:

- one long-lived rust-analyzer process for iterative edits
- cheap verification after each change (no full `cargo test`)
- measured latency from edit event to first diagnostic update

## What it does

The script runs rust-analyzer once, keeps it warm, and reports per-change timing:

- change kind (`whitespace`, `comment`, `code`, or custom label)
- elapsed ms from `didChange` to first `textDocument/publishDiagnostics`
- diagnostic count after that change
- CSV log file for trend comparison

## Setup

1. Ensure rust-analyzer is available:
   - `rustup component add rust-analyzer`
2. Open the target worktree checkout (for example `/home/dev-user/code/codex-lsp`).
3. Use a small interactive file to pilot realistic edit types:
   - comment-only change
   - whitespace-only change
   - code change
   - larger refactor change

## Core script

From your codex root, run:

```bash
python ~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_timing.py \
  /home/dev-user/code/codex-lsp \
  codex-rs/core/src/tools/orchestrator.rs \
  --iterations 0 \
  --log /tmp/ra-lsp-timing.csv
```

`--iterations 0` means run until you stop with `q`.

Workflow:

1. Make a manual edit in the target file.
2. Return to terminal and press Enter.
3. (Optional) enter a label for this edit; press Enter for automatic type inference.
4. The script sends the edit over the same rust-analyzer session and logs timing.

## Useful run modes

- **Quick ad-hoc sweep**
  - Run one file through 10 timed edits: set `--iterations 10`.
- **Manual categories**
  - Keep labels consistent: `comment`, `whitespace`, `code` for cleaner comparison.
- **Baseline check**
  - Add `--install-ra` first if rust-analyzer is missing.

## Notes

- This benchmark is best-effort LSP-level feedback latency and not a replacement for targeted `cargo check` on release-critical changes.
- This does not require editor integration; it talks to rust-analyzer directly over LSP stdin/stdout.

## New: rust-analyzer daemon mode

Use this when you want a long-lived, reconnectable RA process you can reuse from quick checks.

### Files

- `~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_daemon.py`
- `~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_client.py`

### Client flow (recommended)

```bash
python ~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_client.py \
  --workspace /home/dev-user/code/codex-lsp \
  --file codex-rs/core/src/tools/orchestrator.rs \
  --action check
```

This command auto-starts the daemon if it is not already running.

### Query state

```bash
python ~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_client.py \
  --workspace /home/dev-user/code/codex-lsp \
  --action state
```

### Stop daemon

```bash
python ~/.codex/skills/rust-analyzer-lsp-timing/scripts/ra_lsp_client.py \
  --workspace /home/dev-user/code/codex-lsp \
  --action stop
```

### What to expect

- The first check after a file edit starts a diagnostics pass.
- Repeated edits are much faster than restarting RA each time because the same process is reused.
