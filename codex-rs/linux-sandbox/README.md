# codex-linux-sandbox

This crate is responsible for producing:

- a `codex-linux-sandbox` standalone executable for Linux that is bundled with the Node.js version of the Codex CLI
- a lib crate that exposes the business logic of the executable as `run_main()` so that
  - the `codex-exec` CLI can check if its arg0 is `codex-linux-sandbox` and, if so, execute as if it were `codex-linux-sandbox`
  - this should also be true of the `codex` multitool CLI

On Linux, the bubblewrap pipeline uses the vendored bubblewrap path compiled
into this binary.

**Current Behavior**
- Legacy Landlock + mount protections remain available as the legacy pipeline.
- The bubblewrap pipeline is standardized on the vendored path.
- During rollout, the bubblewrap pipeline is gated by the temporary feature
  flag `use_linux_sandbox_bwrap` (CLI `-c` alias for
  `features.use_linux_sandbox_bwrap`; legacy remains default when off).
- When enabled, the bubblewrap pipeline applies `PR_SET_NO_NEW_PRIVS` and a
  seccomp network filter in-process.
- When enabled, the filesystem is read-only by default via `--ro-bind / /`.
- When enabled, writable roots are layered with `--bind <root> <root>`.
- When enabled, protected subpaths under writable roots (for example `.git`,
  resolved `gitdir:`, and `.codex`) are re-applied as read-only via `--ro-bind`.
- When enabled, symlink-in-path and non-existent protected paths inside
  writable roots are blocked by mounting `/dev/null` on the symlink or first
  missing component.
- When enabled, the helper isolates the PID namespace via `--unshare-pid`.
- When enabled, it mounts a fresh `/proc` via `--proc /proc` by default, but
  you can skip this in restrictive container environments with `--no-proc`.

**Notes**
- The CLI surface still uses legacy names like `codex debug landlock`.
