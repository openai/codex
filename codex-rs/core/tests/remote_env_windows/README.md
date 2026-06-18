# Windows remote-environment test

This Bazel-only `test_codex` integration test runs a Windows exec-server fixture
under pinned Wine and exercises the normal model tool-call and remote-execution
path.

## Running the test

```sh
bazel test \
  //codex-rs/core/tests/remote_env_windows:smoke-test \
  --test_output=errors
```

No system Wine is required. Every process gets a fresh `WINEPREFIX` and isolated
wineserver.

## Current limitations

- ConPTY/TTY behavior is not yet covered.
- Wine loads shared objects and PE DLLs at runtime, so the host must still
  provide the declared compatible glibc version.
- The target is intentionally limited to x86-64 for simplicity. It can expand
  if we find aarch64-specific behavior worth testing.
