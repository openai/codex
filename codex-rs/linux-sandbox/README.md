# codex-linux-sandbox

Linux sandbox helper used by Codex process execution on Linux.

This crate produces:
- `codex-linux-sandbox` binary
- library entrypoint (`run_main`) reused by `codex` / `codex-exec` arg0 dispatch

## Runtime behavior

- Default path (flag off): legacy Landlock + mount protections.
- Rollout path (flag on `features.use_linux_sandbox_bwrap`): vendored bubblewrap + in-process seccomp.
- Bubblewrap mode uses read-only root (`--ro-bind / /`) and rebinds writable roots.
- Bubblewrap mode re-protects `.git`, resolved `gitdir:`, and `.codex` under writable roots.
- Bubblewrap mode preflights `/proc` mount and retries with `--no-proc` on known mount failure.
- Bubblewrap mode does not fall back to legacy Landlock on bwrap errors.

## Notes

- CLI/debug surfaces still include legacy names such as `codex debug landlock`.
- See `codex-rs/docs/linux_sandbox.md` for full policy semantics.
