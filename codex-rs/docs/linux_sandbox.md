# Linux Sandbox (Vendored bubblewrap, Landlock legacy)

The Linux sandbox helper (`codex-linux-sandbox`) supports two filesystem
pipelines:
- Vendored bubblewrap (feature-flag opt-in): bubblewrap (`bwrap`) for
  filesystem isolation plus seccomp.
- Legacy: mount-based protections plus Landlock enforcement.

## Requirements

- The bubblewrap pipeline requires a Linux build that enables the vendored
  bubblewrap FFI path.
- During rollout, enable bubblewrap with
  `-c use_linux_sandbox_bwrap=true` (CLI override alias) or
  `features.use_linux_sandbox_bwrap = true` in config.

## Filesystem Semantics (bubblewrap pipeline)

When the bubblewrap pipeline is enabled and disk writes are restricted
(`read-only` or `workspace-write`), the helper builds the filesystem view with
bubblewrap in this order:

1. `--ro-bind / /` makes the entire filesystem read-only.
2. `--bind <root> <root>` re-enables writes for each writable root.
3. `--ro-bind <subpath> <subpath>` re-applies read-only protections under
   writable roots so protected paths win.
4. `--dev-bind /dev/null /dev/null` preserves the common sink.

Writable roots and protected subpaths are derived from
`SandboxPolicy::get_writable_roots_with_cwd()`.

Protected subpaths include:
- top-level `.git` (directory or pointer file),
- the resolved `gitdir:` target for worktrees and submodules, and
- top-level `.codex`.

### Deny-path Hardening

To reduce symlink and path-creation attacks inside writable roots:
- If any component of a protected path is a symlink within a writable root, the
  helper mounts `/dev/null` on that symlink.
- If a protected path does not exist, the helper mounts `/dev/null` on the
  first missing path component (when it is within a writable root).

## Process and Network Semantics

- In the bubblewrap pipeline, the helper isolates the PID namespace via
  `--unshare-pid`.
- In the bubblewrap pipeline, it mounts a fresh `/proc` via `--proc /proc` by
  default.
- In restrictive container environments, you can skip the `/proc` mount with
  the helper flag `--no-proc` while still keeping PID isolation enabled.
- Network restrictions are enforced with seccomp when network access is
  disabled.

## Notes

- The CLI still exposes legacy names such as `codex debug landlock`.
- Landlock remains the default filesystem enforcement pipeline when
  `features.use_linux_sandbox_bwrap` is not enabled.
