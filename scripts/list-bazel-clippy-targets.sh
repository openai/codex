#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

manual_rust_test_targets=""
if [[ "${RUNNER_OS:-}" != "Windows" ]]; then
  # Resolve the dynamic targets before printing anything so callers do not
  # continue with a partial list if `bazel query` fails.
  #
  # The generated manual `*-unit-tests-bin` targets pull in Windows-incompatible
  # V8/Python dependencies under gnullvm, so only include them on platforms
  # where they currently analyze successfully.
  manual_rust_test_targets="$(bazel query 'kind("rust_test rule", attr(tags, "manual", //codex-rs/... except //codex-rs/v8-poc/...))')"
fi

printf '%s\n' \
  "//codex-rs/..." \
  "-//codex-rs/v8-poc:all"

# `--config=clippy` on the `workspace_root_test` wrappers does not lint the
# underlying `rust_test` binaries. Add the internal manual `*-unit-tests-bin`
# targets explicitly so inline `#[cfg(test)]` code is linted like
# `cargo clippy --tests`.
if [[ -n "${manual_rust_test_targets}" ]]; then
  printf '%s\n' "${manual_rust_test_targets}"
fi
