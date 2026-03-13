# codex-core

This crate implements the business logic for Codex. It is designed to be used by the various Codex UIs written in Rust.

## Dependencies

Note that `codex-core` makes some assumptions about certain helper utilities being available in the environment. Currently, this support matrix is:

### macOS

Expects `/usr/bin/sandbox-exec` to be present.

When using the workspace-write sandbox policy, the Seatbelt profile allows
writes under the configured writable roots while keeping `.git` (directory or
pointer file), the resolved `gitdir:` target, and `.codex` read-only.

<<<<<<< HEAD
Network access and filesystem read/write roots are controlled by the split
`FileSystemSandboxPolicy` and `NetworkSandboxPolicy` views. On macOS, Seatbelt
enforces overlapping path rules with the most specific entry winning, so a
broader writable root can still contain nested read-only or denied carveouts.
=======
Network access and filesystem read/write roots are controlled by
`SandboxPolicy`. Seatbelt consumes the resolved policy and enforces it.
>>>>>>> 28e7c4637 (fix: fail closed split windows sandboxing during transform)

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

The Linux sandbox helper prefers `/usr/bin/bwrap` whenever it is available and
falls back to the vendored bubblewrap path otherwise. When `/usr/bin/bwrap` is
missing, Codex also surfaces a startup warning through its normal notification
path instead of printing directly from the sandbox helper.

### Windows

The unelevated restricted-token sandbox supports:

- legacy `ReadOnly` and `WorkspaceWrite` behavior
- full-read split filesystem policies whose writable roots still match the
  legacy `WorkspaceWrite` root set, but need extra read-only carveouts under
  those writable roots

The elevated Windows sandbox still supports only the legacy
`SandboxPolicy`-equivalent path.

Windows still rejects split-only filesystem policies that would require direct
read restriction, explicit unreadable carveouts, reopened writable descendants
under read-only carveouts, or elevated setup/runner support, instead of running
with a weaker sandbox.

### All Platforms

Expects the binary containing `codex-core` to simulate the virtual `apply_patch` CLI when `arg1` is `--codex-run-as-apply-patch`. See the `codex-arg0` crate for details.
