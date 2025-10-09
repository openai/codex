# codex-windows-sandbox

A standalone helper executable that can launch commands inside a Windows sandbox
using the same `SandboxPolicy` JSON representation used across Codex.

## Restricted token sandbox

`codex-windows-sandbox` now executes commands behind a restricted primary token
combined with Windows job objects, temporary filesystem ACL adjustments, and
ephemeral Windows Firewall rules. The high‑level flow is:

1. Duplicate the current process token and call [`CreateRestrictedToken`] with
   the `DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`, and `WRITE_RESTRICTED` flags. We
   explicitly disable dangerous built-in SIDs (Administrators, LocalSystem,
   NetworkService, etc.) and add the `WinRestrictedCodeSid` as the only
   restricted SID. This keeps the sandboxed process in the caller's logon
   session, but it behaves like an unprivileged "write-restricted" token that
   requires explicit ACL grants before any write succeeds.
2. The sandbox computes the writable roots from the supplied
   [`SandboxPolicy`]. Every allowed path receives a temporary "allow" ACE for
   the restricted SID, while read-only subpaths get a matching deny ACE. The
   helper tracks every ACE it adds and revokes them when the sandboxed process
   exits, leaving the original ACLs untouched.
3. A Windows job object created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
   ensures that all descendants are terminated as soon as the sandbox process
   handle is dropped. This guarantees prompt cleanup even when a tool tries to
   daemonise itself.
4. If the policy disables networking, we instantiate `INetFwPolicy2` via COM
   and create a transient `INetFwRule3` that blocks outbound traffic for the
   restricted SID. The rule lives only for the lifetime of the sandbox process
   and is removed automatically on drop (with COM initialisation guarded by a
   dedicated RAII helper).

Because the restricted token remains in the user's original logon session,
commands such as `git`, `python`, `powershell`, or `whoami` continue to behave
as expected. At the same time, attempts to write outside the configured
workspace roots fail with `ACCESS_DENIED`, and outbound connections are refused
while the firewall rule is active.

[`CreateRestrictedToken`]: https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-createrestrictedtoken
[`SandboxPolicy`]: ../protocol/src/protocol.rs
