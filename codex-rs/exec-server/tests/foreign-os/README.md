# Foreign-OS exec-server tests

This directory cross-builds a small Windows exec-server fixture and runs it
under a pinned Wine runtime from x86-64 Linux Bazel. A real Linux app-server
drives it through the normal model tool-call and remote-execution path. The
Windows fixture links only `codex-exec-server` because the full Codex Windows
graph does not yet cross-build.

## Running the test

```sh
bazel test \
  //codex-rs/exec-server/tests/foreign-os:smoke-test \
  --test_output=errors
```

No system Wine or apt packages are required. Every process gets a fresh
`WINEPREFIX` and isolated wineserver.

## Current limitations

- PowerShell and ConPTY/TTY behavior are not yet covered.
- Wine loads shared objects and PE DLLs at runtime, so the host must still
  provide the declared compatible glibc version.
- The target is intentionally limited to x86-64 for simplicity. It can expand
  if we find aarch64-specific behavior worth testing.
