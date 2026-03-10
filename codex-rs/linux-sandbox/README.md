# codex-linux-sandbox

This crate is responsible for producing:

- a `codex-linux-sandbox` standalone executable for Linux that is bundled with the Node.js version of the Codex CLI
- a lib crate that exposes the business logic of the executable as `run_main()` so that
  - the `codex-exec` CLI can check if its arg0 is `codex-linux-sandbox` and, if so, execute as if it were `codex-linux-sandbox`
  - this should also be true of the `codex` multitool CLI

On Linux, the bubblewrap pipeline uses the vendored bubblewrap path compiled
into this binary.

**Current Behavior**
- Bubblewrap is the default filesystem sandbox pipeline and is standardized on
  the vendored path.
- Legacy Landlock + mount protections remain available only as an explicit
  fallback path.
- Split-only filesystem policies that do not round-trip through the legacy
  `SandboxPolicy` model are routed through bubblewrap automatically so nested
  read-only or denied carveouts are preserved.
- When bubblewrap is active, the helper applies `PR_SET_NO_NEW_PRIVS` and a
  seccomp network filter in-process.
- When bubblewrap is active, the filesystem is read-only by default via
  `--ro-bind / /`.
- When bubblewrap is active, writable roots are layered with `--bind <root>
  <root>`.
- When bubblewrap is active, protected subpaths under writable roots (for
  example `.git`,
  resolved `gitdir:`, and `.codex`) are re-applied as read-only via `--ro-bind`.
- When bubblewrap is active, overlapping split-policy entries are applied in
  path-specificity order so narrower writable children can reopen broader
  read-only parents while narrower denied subpaths still win.
- When bubblewrap is active, symlink-in-path and non-existent protected paths inside
  writable roots are blocked by mounting `/dev/null` on the symlink or first
  missing component.
- When bubblewrap is active, the helper explicitly isolates the user namespace via
  `--unshare-user` and the PID namespace via `--unshare-pid`.
- When bubblewrap is active and network is restricted without proxy routing,
  the helper also
  isolates the network namespace via `--unshare-net`.
- In managed proxy mode, the helper uses `--unshare-net` plus an internal
  TCP->UDS->TCP routing bridge so tool traffic reaches only configured proxy
  endpoints.
- In managed proxy mode, after the bridge is live, seccomp blocks new
  AF_UNIX/socketpair creation for the user command.
- When bubblewrap is active, it mounts a fresh `/proc` via `--proc /proc` by default, but
  you can skip this in restrictive container environments with `--no-proc`.

**Notes**
- The CLI surface still uses legacy names like `codex debug landlock`.
