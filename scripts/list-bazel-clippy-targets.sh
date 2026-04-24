#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

# Resolve the dynamic targets before printing anything so callers do not
# continue with a partial list if `bazel query` fails. Reuse the same CI Bazel
# server settings as the subsequent build so Windows jobs do not cold-start a
# second Bazel server just for target discovery.
manual_rust_test_targets="$(
  ./.github/scripts/run-bazel-query-ci.sh \
    --output=label \
    -- 'kind("rust_test rule", attr(tags, "manual", //codex-rs/... except //codex-rs/v8-poc/...))'
)"

printf '%s\n' \
  "//codex-rs/..." \
  "-//codex-rs/v8-poc:all"

# `--config=clippy` on the `workspace_root_test` wrappers does not lint the
# underlying `rust_test` binaries. Add the internal manual `*-unit-tests-bin`
# targets explicitly so inline `#[cfg(test)]` code is linted like
# `cargo clippy --tests`.
printf '%s\n' "${manual_rust_test_targets}"
