# Foreign-OS exec-server tests

This directory cross-builds a small Windows exec-server fixture and runs it
under a pinned Wine runtime from x86-64 Linux Bazel. A real Linux app-server
drives it through the normal model tool-call and remote-execution path. The
Windows fixture links only `codex-exec-server` because the full Codex Windows
graph does not yet cross-build, and the smaller target is faster to iterate on.
The test intentionally records the current host-shell mismatch: app-server
chooses a Unix shell on Linux, so the Windows remote command fails before
running.

## Running the test

```sh
bazel test \
  //codex-rs/exec-server/tests/foreign-os:smoke-test \
  --test_output=errors
```

No system Wine or apt packages are required. The target is intentionally
limited to x86-64 for simplicity, so Bazel marks it incompatible elsewhere.
Every process gets a fresh `WINEPREFIX` and isolated wineserver.

The reusable rules and Rust harness live under `//bazel/rules/testing`. Tests
resolve their Windows executable with `codex_utils_cargo_bin::cargo_bin`, then
use `WineTestProcess::scope` for async teardown and panic-safe cleanup.

## Current limitations

- PowerShell and ConPTY/TTY behavior are not yet covered.
- Wine loads shared objects and PE DLLs at runtime, so the host must still
  provide the declared compatible glibc version.
- ARM64 Linux is intentionally unsupported for now. It is more complicated to
  support, and the behavior covered here should not vary by architecture.
