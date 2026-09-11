# Native Windows MXC sandbox

This crate routes a command through the current Codex executable and directly
into Microsoft's MXC `BaseContainerRunner`. It requires a working Windows
process security environment (PSEC). It never invokes MXC's AppContainer
dispatcher, edits host ACLs, creates sandbox users, runs setup, or requests
elevation. The existing Codex Windows sandboxes remain separate backends.

Windows executors record `codex.windows_mxc.available` once per process with an
`available=true|false` tag. This measures capability independently of selection.
The exec-server handshake also exposes `windows_mxc_available`; older or
unsupported Windows executors reject MXC requests before execution.

`is_available()` uses MXC's cached create/close probe, rather than an OS build
number or the SDK's broad `platform_support()` result. The latter also reports
older AppContainer backends as supported. A requested deny path additionally
requires the native `PSE_SUPPORT_FS_DENY` capability; otherwise the command
fails before launch.

The wrapper inherits the command's pipes or ConPTY console. MXC creates its
child suspended, assigns a kill-on-close job before resuming it, and retains
the native policy through workload completion. Filesystem permissions come
from the canonical Codex permission profile, including protected metadata
carveouts. Supported managed network access allows IPv4 and IPv6 loopback
clients and servers, including the dedicated proxy listeners, while denying
direct non-loopback egress and general inbound network access.
Win32k calls and desktop handles remain available for PowerShell startup;
clipboard, input-injection, and desktop/system-control restrictions remain.

## Launch contract

`create_command_args()` wraps the command like the Seatbelt backend, using the
executor's Codex executable as the native SDK helper. It carries one typed
request: the canonical `PermissionProfile`, policy cwd, proxy context, and exact
argv. The helper inherits command cwd, environment, and stdio from the shared
process path; policy cwd can differ from command cwd.

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
- This adapter rejects managed networking with `allow_local_binding=false`:
  its host-loopback permission is bidirectional. MXC's proxy-peer identity mode
  is not integrated here. With local binding enabled, direct DNS remains denied,
  matching the existing Windows sandbox.
- Windows volume-root grants do not recurse. The adapter grants the root and
  its immediate children; directories added or newly mounted during a running
  command are not implicitly granted.
- The native API represents paths and environment values as Unicode strings.
  Non-Unicode values fail instead of undergoing lossy conversion.
- An explicitly empty child environment is rejected: the SDK replaces an empty
  environment list with profile defaults and has no explicit-empty option.
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
