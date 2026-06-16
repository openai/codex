---
name: remote-tests
description: How to run tests using remote executor.
---

Some Codex integration tests select `local`, `docker`, or `wine-exec` through
`CODEX_TEST_ENVIRONMENT`. The legacy `CODEX_TEST_REMOTE_ENV=<container>` still
selects Docker; otherwise execution is local.

Docker container is built and initialized via ./scripts/test-remote-env.sh

On x86-64 Linux, run Wine exec with
`bazel test //codex-rs/core:core-all-wine-exec-test --test_output=errors`.
Temporary blockers belong beside the test in `skip_if_wine_exec!` calls.

You can list devboxes via `applied_devbox ls`, pick the one with `codex` in the name.
Connect to devbox via `ssh <devbox_name>`.
Reuse the same checkout of codex in `~/code/codex`. Reset files if needed. Multiple checkouts take longer to build and take up more space.
Check whether the SHA and modified files are in sync between remote and local.
