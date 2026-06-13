# Windows remote-environment tests

This directory cross-builds a small Windows exec-server fixture and runs it
under a pinned Wine runtime from x86-64 Linux Bazel. The Windows fixture links
only `codex-exec-server` because the full Codex Windows graph does not yet
cross-build, and the smaller target is faster to iterate on. The direct smoke
test runs pinned PowerShell and built-in `cmd.exe` through the real exec-server
with Windows cwd URI values.

## Running the test

```sh
bazel test \
  //codex-rs/core/tests/remote_env_windows:smoke-test \
  --test_output=errors
```

The full cross-OS flow uses a real Linux app-server for two turns against the
same Windows exec-server:

```sh
bazel test \
  //codex-rs/core/tests/remote_env_windows:wine-app-server-windows-exec-server-test \
  --test_output=errors
```

No system Wine or apt packages are required. The target is intentionally
limited to x86-64 for simplicity, so Bazel marks it incompatible elsewhere.
Every process gets a fresh `WINEPREFIX` and isolated wineserver.

The reusable rules and Rust harness live under `//bazel/rules/testing`. Tests
resolve their Windows executable with `codex_utils_cargo_bin::cargo_bin`, then
use `WineTestProcess::scope` for async teardown and panic-safe cleanup.

## Current limitations

- ConPTY/TTY behavior is not yet covered.
- Wine loads shared objects and PE DLLs at runtime, so the host must still
  provide the declared compatible glibc version.
- ARM64 Linux is intentionally unsupported for now. It is more complicated to
  support, and the behavior covered here should not vary by architecture.
