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

Seatbelt also keeps the legacy default preferences read access
(`user-preference-read`) needed for cfprefs-backed macOS behavior.

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

The Linux sandbox helper prefers the first `bwrap` found on `PATH` outside the
current working directory whenever it is available. If `bwrap` is present but
too old to support `--argv0`, the helper keeps using system bubblewrap and
switches to a no-`--argv0` compatibility path for the inner re-exec. If
`bwrap` is missing, it falls back to the vendored bubblewrap path compiled into
the binary and Codex surfaces a startup warning through its normal notification
path instead of printing directly from the sandbox helper.

### Windows

Legacy `SandboxPolicy` / `sandbox_mode` configs are still supported on
Windows.

The elevated setup/runner backend supports legacy `ReadOnlyAccess::Restricted`
for `read-only` and `workspace-write` policies. Restricted read access honors
explicit readable roots plus the command `cwd`, and keeps writable roots
readable when `workspace-write` is used.

When `include_platform_defaults = true`, the elevated Windows backend adds
backend-managed system read roots required for basic execution, such as
`C:\Windows`, `C:\Program Files`, `C:\Program Files (x86)`, and
`C:\ProgramData`. When it is `false`, those extra system roots are omitted.

The unelevated restricted-token backend still supports the legacy full-read
Windows model for legacy `ReadOnly` and `WorkspaceWrite` behavior. It also
supports a narrow split-filesystem subset: full-read split policies whose
writable roots still match the legacy `WorkspaceWrite` root set, but add extra
read-only carveouts under those writable roots.

New `[permissions]` / split filesystem policies remain supported on Windows
only when they round-trip through the legacy `SandboxPolicy` model without
changing semantics. Policies that would require direct read restriction,
explicit unreadable carveouts, reopened writable descendants under read-only
carveouts, different writable root sets, or split carveout support in the
elevated setup/runner backend still fail closed instead of running with weaker
enforcement.

### All Platforms

Expects the binary containing `codex-core` to simulate the virtual `apply_patch` CLI when `arg1` is `--codex-run-as-apply-patch`. See the `codex-arg0` crate for details.
