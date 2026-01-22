# Windows Sandbox Security Details

For overall context on sandboxing in Codex, see [sandbox.md](./sandbox.md).

## Implementation Overview

When commands run via `codex sandbox windows …` (or when the CLI/TUI calls into the same crate in-process for sandboxed turns), the launcher configures a restricted Windows token and an allowlist policy scoped to the declared workspace roots. Writes are blocked everywhere except inside those roots (plus `%TEMP%` when workspace-write mode is requested), and common escape vectors such as alternate data streams, UNC paths, and device handles are denied proactively. The CLI also injects stub executables (for example, wrapping `ssh`) ahead of the host PATH so we can intercept dangerous tools before they ever leave the sandbox.

## Known Security Limitations

Running `python windows-sandbox-rs/sandbox_smoketests.py` with full filesystem and network access currently results in **37/41** passing cases. The list below focuses on the four high-value failures numbered #32 and higher in the smoketests (earlier tests are less security-focused).

| Test                                          | Purpose                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ADS write denied (#32)                        | Ensures alternate data streams cannot be written inside the workspace, preventing tools from hiding payloads in ADS. The sandbox currently allows the write (process returns `rc=0`).                                                                                                                        |
| protected path case-variation denied (#33)    | Confirms that protected directories such as `.git` remain blocked even when attackers use case tricks like `.GiT`. The current allowlist treats `.GiT` as distinct, so the write succeeds.                                                                                                                   |
| PATH stub bypass denied (#35)                 | Verifies that a workspace-provided `ssh.bat` shim placed ahead of the host PATH runs instead of the real `ssh`. The sandbox exits early before emitting the shim's `stubbed` output, so we cannot prove the interception works.                                                                              |
| Start-Process https denied (KNOWN FAIL) (#41) | Validates that read-only runs cannot launch the host's default browser via `Start-Process 'https://...'`. Today the command succeeds (exit code 0) because Explorer handles the ShellExecute request outside the sandbox. The failure is captured by `windows-sandbox-rs/sandbox_smoketests.py` (last case). |

## Want to Help?

If you are a security-minded Windows user, help us get these tests passing! Improved implementations that make these smoke tests pass meaningfully reduce Codex's escape surface. After iterating, rerun `python windows-sandbox-rs/sandbox_smoketests.py` to validate the fixes and help us drive the suite toward 41/41.
