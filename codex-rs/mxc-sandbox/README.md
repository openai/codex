# Native Windows MXC sandbox

This crate routes a command through the current Codex executable and directly
into Microsoft's public MXC V1 SDK with inherited stdio. It requires a working
Windows process security environment (PSEC). It never invokes MXC's AppContainer
dispatcher, edits host ACLs, creates sandbox users, runs setup, or requests
elevation. The existing Codex Windows sandboxes remain separate backends.

Windows executors record `codex.windows_mxc.available` once per process with an
`available=true|false` tag. This measures runtime availability independently of
selection. Unsupported Windows executors reject MXC requests before execution.

`is_available()` caches the public SDK's ProcessContainer probe. This checks
API presence and capabilities without creating a security environment; creation
and request-specific failures remain launch errors. The SDK rejects requested
deny paths when the host cannot enforce them.

The wrapper inherits the command's pipes or ConPTY console. MXC creates its
child suspended, assigns a kill-on-close job before resuming it, and retains
the native policy through workload completion. Filesystem permissions come
from the canonical Codex permission profile, including protected metadata
carveouts. Supported managed network access allows IPv4 and IPv6 loopback
clients and servers, including the dedicated proxy listeners, while denying
direct non-loopback egress and general inbound network access.
Win32k calls and desktop handles remain available for PowerShell startup;
clipboard, input-injection, and desktop/system-control restrictions remain.

Filesystem conflicts in the emitted policy are enforced by BaseContainer:
equivalent path spellings and symlink/junction aliases use the strictest access
(`deny` > read-only > read-write), while more-specific paths override ancestor
grants. See the [BaseContainer path-precedence rules](https://github.com/microsoft/mxc/blob/main/docs/schema.md#overlapping-paths-for-windows-basecontainer).

Do not call MXC's shared `normalize_object_conflicts` before or after volume
expansion. Its identity preflight is used by mount-based backends and can reject
an otherwise usable Windows policy when a network share does not support
`FileIdInfo`, or a protected deny path cannot be opened for metadata. Codex sends
explicit path grants without that object-identity lookup. The public SDK does
perform metadata checks for missing paths: immediate failures produce warnings,
while a check that exceeds five seconds rejects startup, including on a slow or
unreachable network share.
Hard-link names retain independent path permissions. An allowed name can read
or modify the same file as a denied or read-only name.

Before expanding a writable volume root, the adapter checks whether a restricted
directory names that root. For example, a read-only junction to writable `V:\`
suppresses the generated write grant for `V:\projects`. Explicit grants beneath
the root still apply. A child junction on another writable drive gets no generated
write grant if it points beneath that restriction. When a deny names the same
writable volume through another path, the adapter rejects the request before
launch. Otherwise generated read-only grants such as `V:\.aws` could expose
files beneath the denied root. These checks use the links present during
policy construction.

If the filesystem cannot supply a directory ID, the adapter compares resolved
NT names. Different share names can refer to the same directory. If the adapter
cannot distinguish a writable volume from a restricted directory, it reports both
paths and stops before launch. Explicit grants outside volume expansion require
no directory-identity comparison.

## Selection and rollout

`windows.sandbox = "mxc"` is strict. The default-off `features.prefer_mxc`
selects MXC for local execution when available unless local binding is explicitly
forbidden; otherwise legacy configuration and setup apply. Remote executors keep
their configured backend. Command failures never trigger backend fallback.
Desktop can supply `-c features.prefer_mxc=true` from its saved rollout decision
on the next local Windows launch. Gate changes require a subsequent launch, and
rollback may require legacy setup. `config/read` retains the configured sandbox;
`windowsSandbox/readiness` reflects the effective local selection.

## Launch contract

`create_command_args()` wraps the command like the Seatbelt backend, using the
executor's Codex executable as the native SDK helper. It carries one typed
request: the canonical `PermissionProfile`, policy cwd, proxy context, and exact
argv. The helper inherits command cwd, environment, and stdio from the shared
process path; policy cwd can differ from command cwd.

Windows MXC reserves `SystemDrive` and `LOCALAPPDATA` for the host's validated,
nonempty values in both wrapper and explicit child environments. Caller
overrides are replaced case-insensitively; unavailable host values fail before
launch with a diagnostic naming the variable.
Windows may virtualize the child's `LOCALAPPDATA` into a sandbox directory
beneath the host profile.

The request uses launcher-only environment chunks to leave Windows' command-line
budget to the workload. Payloads are limited to 1,000,000 UTF-8 bytes and chunks
to 4096 bytes; the helper removes them before native process creation. Use
`codex sandbox windows` to debug through the same preparation path as execution.

## Limits and Windows validation

- Deny globs use the existing Windows sandbox resolver to expand matching files
  and directories into concrete paths before launch. This has the same snapshot
  semantics and scan limits as the existing Windows sandbox.
- Native deny paths depend on the host's capability probe. An installed Windows
  update alone is not treated as evidence that every policy feature is enabled.
- When MXC is the executor's selected backend, managed networking defaults
  `allow_local_binding` to `true`. An effective `false` after applying managed
  requirements is a configuration error: native host-loopback access
  is bidirectional, and MXC's proxy-peer identity mode is not integrated here.
  `true` permits local servers and direct host-loopback connections and removes
  the proxy's additional private-network destination checks. Proxy domain rules
  still apply to proxied traffic; direct DNS remains denied. This default also
  applies to remote Windows executors and does not enable disabled networking.
- Read-only volume-root grants recurse without enumerating children. Writable
  volume-root grants do not recurse, so the adapter grants the root and its
  immediate children; new children added during a running command are not
  implicitly writable.
- Default read access excludes network shares unrelated to the policy or command
  cwd. Explicit read/write grants remain supported. Unrelated shares are not
  probed for object identity or enumerated for implicit reads.
- The native API represents paths and environment values as Unicode strings.
  Non-Unicode values fail instead of undergoing lossy conversion.
- An empty child environment still uses the Windows user-profile defaults.
  Supplying only two keys would turn it into an incomplete explicit environment,
  so the wrapper's bootstrap values are removed before the SDK selects defaults.
  Nonempty environments remain explicit except for these two host-owned values.
- The upstream runner terminates remaining descendants when the foreground
  process exits, as well as on cancellation. Both existing Windows backends
  preserve descendants after normal exit, so detached servers currently lose
  that behavior under MXC. Retaining descendants safely requires a longer-lived
  owner for the native policy and job.
- The command itself remains subject to Windows' command-line length limit.
- Portable tests validate policy translation and wrapper arguments. Actual
  enforcement, nested access overrides, alternate path encodings, junctions
  and hardlinks, protected metadata, ConPTY behavior, process-tree cancellation,
  proxy isolation, and
  comparison with the existing Windows and Unix sandboxes require the smoke
  suite on supported Windows and the corresponding platform hosts.
  Test the normal `powershell.exe` and `pwsh.exe` command paths, not only `cmd.exe`.

The MXC git revision is pinned with the workspace dependencies. Native launch
errors are returned without dumping the SDK's diagnostic buffer, which may
contain command or environment data.
