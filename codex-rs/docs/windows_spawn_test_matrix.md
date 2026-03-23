# Windows Spawn Test Matrix

This document is a practical playbook for a Codex agent testing Windows command
spawning across:

- one-shot vs `unified_exec`
- no sandbox vs legacy sandbox vs elevated sandbox
- `tty=false` vs `tty=true` for `unified_exec`

The goal is not perfect exhaustiveness. The goal is to hit the major spawn
architectures with cheap, repeatable checks that catch the most likely
regressions.

## What This Matrix Covers

There are 9 meaningful execution modes on Windows:

| Mode | Sandbox | TTY |
| --- | --- | --- |
| one-shot | none | n/a |
| one-shot | legacy (`unelevated`) | n/a |
| one-shot | elevated | n/a |
| `unified_exec` | none | `false` |
| `unified_exec` | none | `true` |
| `unified_exec` | legacy (`unelevated`) | `false` |
| `unified_exec` | legacy (`unelevated`) | `true` |
| `unified_exec` | elevated | `false` |
| `unified_exec` | elevated | `true` |

On Windows, the high-risk split is:

- direct parent -> child spawn
- parent -> runner IPC -> child spawn

That is why elevated sandbox needs extra attention.

## Preflight

Run these before doing any manual testing:

```powershell
cargo build -p codex-cli -p codex-windows-sandbox
cargo test -p codex-windows-sandbox
cargo test -p codex-core unified_exec -- --nocapture
```

If you changed `windows-sandbox-rs`, rebuild the runner before trusting any
manual CLI result:

```powershell
cargo build -p codex-windows-sandbox
```

If you plan to test the TUI interactively, start with tracing and a dedicated
log directory:

```powershell
$env:RUST_LOG="trace"
cargo run --bin codex -- -c log_dir=\"C:\\temp\\codex-spawn-logs\"
```

## Config Modes

Pick exactly one Windows sandbox mode for each test pass.

### No Sandbox

Do not set `[windows].sandbox`. Also ensure the legacy feature toggles are not
forcing a Windows sandbox mode.

### Legacy Sandbox

```toml
[windows]
sandbox = "unelevated"
```

### Elevated Sandbox

```toml
[windows]
sandbox = "elevated"
```

### Enable `unified_exec`

```toml
[features]
unified_exec = true
```

Restart Codex after changing config.

## Recommended Workflow

Use two lanes:

- primary lane: the currently running TUI/harness session
- secondary lane: `codex exec` for the other Windows sandbox mode

The TUI session should test whichever sandbox mode is already active for that
Codex process. Do not assume the agent can safely flip the live session between
legacy and elevated modes.

If the active TUI session is:

- elevated, use the TUI for elevated checks and `codex exec` for legacy checks
- legacy, use the TUI for legacy checks and `codex exec` for elevated checks
- unsandboxed, use the TUI for no-sandbox checks and use `codex exec` for both
  legacy and elevated spot checks

## General Rules

- Use a fresh Codex process after changing config.
- For `unified_exec`, always verify the tool surface first.
- Do not trust a TUI session that reports only `functions.shell_command`.
- For `unified_exec`, the expected shell tools are `exec_command` and
  `write_stdin`.
- For elevated paths, run the same case twice. The second run helps catch
  helper materialization and stale-runner issues.
- Prefer `workspace-write` when testing sandboxed paths. It exercises more of
  the Windows ACL/capability setup than `read-only`.

## Fast Smoke Commands

Use these repeatedly across the matrix.

### Stdout + CWD

This is the cheapest smoke test for basic launch, env, and cwd handling.

```text
pwsh -NoProfile -Command "Write-Output hello; Get-Location"
```

Expected:

- exits successfully
- prints `hello`
- reports the expected working directory

### EOF-Sensitive Non-TTY Command

This catches cases where non-interactive stdin is incorrectly left open.

```text
cmd /c "findstr ."
```

Expected:

- exits promptly
- produces no output
- does not hang waiting for stdin

### Long-Lived PTY Session

This catches session reuse, PTY startup, stdin forwarding, and exit propagation.

Start with:

```text
pwsh -NoLogo -NoProfile -NoExit -Command "$PID; Write-Output ready"
```

Then send:

```text
Write-Output second
```

Then send:

```text
exit
```

Expected:

- first call prints a process id and `ready`
- second call reuses the same process id
- second call prints `second`
- final `exit` closes the session cleanly

## One-Shot Matrix

Run the one-shot checks in the TUI for the active sandbox mode. Use
`cargo run --bin codex -- exec ...` only for the other sandbox mode that the
current TUI session is not exercising.

### 1. One-Shot, No Sandbox

Suggested command if you are using `codex exec` for this lane:

```powershell
cargo run --bin codex -- exec --skip-git-repo-check --sandbox danger-full-access "pwsh -NoProfile -Command \"Write-Output hello; Get-Location\""
```

Checks:

- basic unsandboxed spawn
- PowerShell launch
- cwd propagation

### 2. One-Shot, Legacy Sandbox

Config:

```toml
[windows]
sandbox = "unelevated"
```

Suggested command if this is the secondary `codex exec` lane:

```powershell
cargo run --bin codex -- exec --skip-git-repo-check --sandbox workspace-write "pwsh -NoProfile -Command \"Write-Output hello; Get-Location\""
```

Checks:

- restricted-token spawn
- workspace-write ACL setup
- PowerShell launch
- cwd propagation

Follow with:

```powershell
cargo run --bin codex -- exec --skip-git-repo-check --sandbox workspace-write "cmd /c \"findstr .\""
```

Checks:

- stdin closes for one-shot non-interactive commands

### 3. One-Shot, Elevated Sandbox

Config:

```toml
[windows]
sandbox = "elevated"
```

Run both of these twice if this is the secondary `codex exec` lane:

```powershell
cargo run --bin codex -- exec --skip-git-repo-check --sandbox workspace-write "pwsh -NoProfile -Command \"Write-Output hello; Get-Location\""
cargo run --bin codex -- exec --skip-git-repo-check --sandbox workspace-write "cmd /c \"findstr .\""
```

Checks:

- runner helper resolution/materialization
- parent <-> runner IPC
- PowerShell startup
- non-interactive stdin closure
- repeated elevated runs

## `unified_exec` Matrix

These should be run in the live TUI session for the currently active sandbox
mode. Use `codex exec` only if you need a deterministic fallback or you need to
cross-check the other sandbox mode without restarting the TUI.

Start Codex with:

```powershell
$env:RUST_LOG="trace"
cargo run --bin codex -- -c log_dir=\"C:\\temp\\codex-spawn-logs\"
```

Before any real test, ask:

```text
List the exact names of the shell-related tools you have.
```

Expected:

- `exec_command`
- `write_stdin`

If the session does not report those tools, stop. Do not trust the rest of the
`unified_exec` run.

### Common TTY-False Prompt

Use this prompt for non-TTY cases:

```text
Do not inspect the repository. Use exec_command with tty=false to run `cmd /c "findstr ."` and report whether it exits promptly. Do not use write_stdin.
```

For the current `exec_command` tool surface, `tty=false` means plain pipes with
stdin closed after launch. It does not mean "non-PTY but keep stdin open for
later writes". That richer mode exists in app-server as `stream_stdin=true`,
but it is not part of the `exec_command` matrix yet. So for parity with the
current macOS/CLI harness behavior, treat `tty=false` as a one-call check and
do not send `write_stdin`.

Expected:

- no hang
- prompt exit
- no output or trivial empty output

### Common TTY-True Prompt

Use this prompt for PTY cases:

```text
Do not inspect the repository. Use exec_command with tty=true to start `pwsh -NoLogo -NoProfile -NoExit -Command "$PID; Write-Output ready"`. Then use write_stdin to send `Write-Output second`, then use write_stdin again to send `exit`. Report the reused process id and the outputs from all three calls.
```

Expected:

- first tool call returns a process id and `ready`
- second tool call reuses the same process id
- second tool call prints `second`
- third tool call exits the session

### 4. `unified_exec`, No Sandbox, `tty=false`

Config:

- no `[windows].sandbox`
- `[features].unified_exec = true`

Run the common `tty=false` prompt in the TUI if no sandbox is the active mode.

Checks:

- no-sandbox pipe path
- EOF behavior for non-TTY

### 5. `unified_exec`, No Sandbox, `tty=true`

Config:

- no `[windows].sandbox`
- `[features].unified_exec = true`

Run the common `tty=true` prompt in the TUI if no sandbox is the active mode.

Checks:

- direct ConPTY path
- session reuse
- `write_stdin`

### 6. `unified_exec`, Legacy Sandbox, `tty=false`

Config:

```toml
[windows]
sandbox = "unelevated"

[features]
unified_exec = true
```

Run the common `tty=false` prompt in the TUI if legacy sandbox is the active
mode.

Checks:

- restricted-token pipe session path
- stdin-closed behavior
- workspace-write ACLs

### 7. `unified_exec`, Legacy Sandbox, `tty=true`

Config:

```toml
[windows]
sandbox = "unelevated"

[features]
unified_exec = true
```

Run the common `tty=true` prompt in the TUI if legacy sandbox is the active
mode.

Checks:

- restricted-token ConPTY session path
- session reuse
- stdin normalization

### 8. `unified_exec`, Elevated Sandbox, `tty=false`

Config:

```toml
[windows]
sandbox = "elevated"

[features]
unified_exec = true
```

Run the common `tty=false` prompt in the TUI if elevated sandbox is the active
mode.

Checks:

- parent -> runner IPC -> pipe child path
- stdin closes immediately for non-TTY
- no hang on EOF-sensitive commands

### 9. `unified_exec`, Elevated Sandbox, `tty=true`

Config:

```toml
[windows]
sandbox = "elevated"

[features]
unified_exec = true
```

Run the common `tty=true` prompt in the TUI if elevated sandbox is the active
mode.

Checks:

- parent -> runner IPC -> ConPTY child path
- session reuse across runner IPC
- stdin forwarding
- terminate/exit propagation

This is the highest-risk Windows path. If time is limited, do not skip this one.

## Failure Triage

Use this as a quick first guess when a case fails.

### Only Elevated Cases Fail

Look at:

- `windows-sandbox-rs/src/elevated_impl.rs`
- `windows-sandbox-rs/src/elevated/command_runner_win.rs`
- `windows-sandbox-rs/src/elevated/ipc_framed.rs`
- `windows-sandbox-rs/src/elevated/runner_pipe.rs`

### Only `tty=true` Cases Fail

Look at:

- `windows-sandbox-rs/src/conpty/mod.rs`
- `windows-sandbox-rs/src/conpty/proc_thread_attr.rs`
- `windows-sandbox-rs/src/unified_exec/session.rs`
- `utils/pty/src/win/conpty.rs`

### Only Legacy Sandbox Cases Fail

Look at:

- `windows-sandbox-rs/src/lib.rs`
- `windows-sandbox-rs/src/spawn_prep.rs`
- `windows-sandbox-rs/src/token.rs`
- ACL and capability helpers under `windows-sandbox-rs/src/`

### Plain Stdout/CWD Works but EOF-Sensitive Commands Fail

Look at:

- `windows-sandbox-rs/src/process.rs`
- `windows-sandbox-rs/src/winutil.rs`
- helper resolution under `CODEX_HOME/.sandbox-bin`

### Non-TTY Commands Hang

Look at:

- whether the session is incorrectly assuming app-server-style `stream_stdin`
  semantics for `exec_command`
- `stdin_open` handling
- pipe-mode stdin closure
- non-TTY `write_stdin` assumptions

### TUI Does Not Expose `exec_command` / `write_stdin`

Look at:

- `[features].unified_exec`
- the actual tool catalog for the session
- `core/src/tools/spec.rs`

Do not spend time debugging Windows spawn behavior until the tool surface is
correct.

## Minimum Acceptable Test Pass

If time is short, at least run these:

1. `cargo test -p codex-windows-sandbox`
2. one-shot elevated `pwsh -NoProfile -Command "Write-Output hello; Get-Location"`
3. one-shot elevated `cmd /c "findstr ."`
4. `unified_exec` elevated `tty=false` with `cmd /c "findstr ."`
5. `unified_exec` elevated `tty=true` with the persistent PowerShell session

Those five checks cover the highest-risk Windows spawn surfaces.

## Practical Pattern

If you are running a live TUI session and want broad coverage with minimal
restarts, use this pattern:

1. Detect which Windows sandbox mode the live TUI is already using.
2. Run all relevant one-shot and `unified_exec` checks for that mode in the TUI.
3. Use `codex exec` to run the equivalent one-shot spot checks for the other
   Windows sandbox mode.
4. Only restart the TUI into the other mode if a failure looks specific to the
   interactive experience rather than the backend spawn path.
