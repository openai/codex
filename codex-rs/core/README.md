# codex-core

This crate implements the business logic for Codex. It is designed to be used by the various Codex UIs written in Rust.

## Dependencies

Note that `codex-core` makes some assumptions about certain helper utilities being available in the environment. Currently, this support matrix is:

### macOS

Expects `/usr/bin/sandbox-exec` to be present.

When using the workspace-write sandbox policy, the Seatbelt profile allows
writes under the configured writable roots while keeping `.git` (directory or
pointer file), the resolved `gitdir:` target, and `.codex` read-only.

Network access and filesystem read/write roots are controlled by
`SandboxPolicy`. Seatbelt consumes the resolved policy and enforces it.

Seatbelt also supports macOS permission-profile extensions layered on top of
`SandboxPolicy`:

- no extension profile provided:
  keeps legacy default preferences read access (`user-preference-read`).
- extension profile provided with no `macos_preferences` grant:
  does not add preferences access clauses.
- `macos_preferences = "readonly"`:
  enables cfprefs read clauses and `user-preference-read`.
- `macos_preferences = "readwrite"`:
  includes readonly clauses plus `user-preference-write` and cfprefs shm write
  clauses.
- `macos_automation = true`:
  enables broad Apple Events send permissions.
- `macos_automation = ["com.apple.Notes", ...]`:
  enables Apple Events send only to listed bundle IDs.
- `macos_launch_services = true`:
  enables LaunchServices lookups and open/launch operations.
- `macos_accessibility = true`:
  enables `com.apple.axserver` mach lookup.
- `macos_calendar = true`:
  enables `com.apple.CalendarAgent` mach lookup.
- `macos_contacts = "read_only"`:
  enables Address Book read access and Contacts read services.
- `macos_contacts = "read_write"`:
  includes the readonly Contacts clauses plus Address Book writes and keychain/temp helpers required for writes.

### Linux

Expects the binary containing `codex-core` to run the equivalent of `codex sandbox linux` (legacy alias: `codex debug landlock`) when `arg0` is `codex-linux-sandbox`. See the `codex-arg0` crate for details.

Legacy `SandboxPolicy` / `sandbox_mode` configs are still supported on Linux.
They can continue to use the legacy Landlock path when the split filesystem
policy is sandbox-equivalent to the legacy model after `cwd` resolution.

Split filesystem policies that need direct `FileSystemSandboxPolicy`
enforcement, such as read-only or denied carveouts under a broader writable
root, automatically route through bubblewrap. The legacy Landlock path is used
only when the split filesystem policy round-trips through the legacy
`SandboxPolicy` model without changing semantics. That includes overlapping
cases like `/repo = write`, `/repo/a = none`, `/repo/a/b = write`, where the
more specific writable child must reopen under a denied parent.

### Windows

Legacy `SandboxPolicy` / `sandbox_mode` configs are still the supported Windows
baseline.

The restricted-token sandbox currently enforces only the subset of filesystem
and network restrictions that round-trip through the legacy `SandboxPolicy`
model. Split-only filesystem carveouts that require direct
`FileSystemSandboxPolicy` enforcement are rejected instead of running with a
weaker sandbox.

### All Platforms

Expects the binary containing `codex-core` to simulate the virtual `apply_patch` CLI when `arg1` is `--codex-run-as-apply-patch`. See the `codex-arg0` crate for details.
