# Exec-server test support

## Linux bwrap exec-server

The bwrap fixture runs Linux exec-server in an RBE-isolated outer namespace,
then checks that the production sandbox can create nested namespaces, a fresh
`/proc`, and filesystem and network restrictions. Run both contract layers with:

```
bazel test --config=buildbuddy-openai-rbe //bazel/rules/testing/bwrap:bwrap-test-support-smoke-test --test_output=errors
bazel test --config=buildbuddy-openai-rbe //codex-rs/exec-server/testing:bwrap-exec-server-smoke-test --test_output=errors
```

## Windows exec-server fixture

This directory contains the small Windows exec-server binary used by
foreign-OS tests. It links only `codex-exec-server` because the full Codex
Windows graph does not yet cross-build with Bazel.
