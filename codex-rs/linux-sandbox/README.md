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
- The default Linux sandbox pipeline is bubblewrap on the vendored path.
- Set `features.use_legacy_landlock = true` (or CLI `-c use_legacy_landlock=true`)
  to force the legacy Landlock fallback.
- When the default bubblewrap pipeline is active, it applies `PR_SET_NO_NEW_PRIVS` and a
  seccomp network filter in-process.
- When the default bubblewrap pipeline is active, the filesystem is read-only by default via `--ro-bind / /`.
- When the default bubblewrap pipeline is active, writable roots are layered with `--bind <root> <root>`.
- When the default bubblewrap pipeline is active, protected subpaths under writable roots (for example `.git`,
  resolved `gitdir:`, and `.codex`) are re-applied as read-only via `--ro-bind`.
- When the default bubblewrap pipeline is active, symlink-in-path and non-existent protected paths inside
  writable roots are blocked by mounting `/dev/null` on the symlink or first
  missing component.
- When the default bubblewrap pipeline is active, the helper explicitly isolates the user namespace via
  `--unshare-user` and the PID namespace via `--unshare-pid`.
- When the default bubblewrap pipeline is active and network is restricted without proxy routing, the helper also
  isolates the network namespace via `--unshare-net`.
- In managed proxy mode, the helper uses `--unshare-net` plus an internal
  TCP->UDS->TCP routing bridge so tool traffic reaches only configured proxy
  endpoints.
- In managed proxy mode, after the bridge is live, seccomp blocks new
  AF_UNIX/socketpair creation for the user command.
- When the default bubblewrap pipeline is active, it mounts a fresh `/proc` via `--proc /proc` by default, but
  you can skip this in restrictive container environments with `--no-proc`.

**Notes**
- The CLI surface still uses legacy names like `codex debug landlock`.
