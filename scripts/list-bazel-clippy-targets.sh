#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

windows_cross_compile=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --windows-cross-compile)
      windows_cross_compile=1
      shift
      ;;
    *)
      echo "Usage: $0 [--windows-cross-compile]" >&2
      exit 1
      ;;
  esac
done

# Resolve the dynamic targets before printing anything so callers do not
# continue with a partial list if `bazel query` fails. Reuse the same CI Bazel
# server settings as the subsequent build so Windows jobs do not cold-start a
# second Bazel server just for target discovery.
if [[ $windows_cross_compile -eq 1 ]]; then
  manual_rust_test_targets="$(
    ./.github/scripts/run-bazel-query-ci.sh \
      --windows-cross-compile \
      --output=label \
      -- 'kind("rust_test rule", attr(tags, "manual", //codex-rs/... except //codex-rs/v8-poc/...))'
  )"
else
  manual_rust_test_targets="$(
    ./.github/scripts/run-bazel-query-ci.sh \
      --output=label \
      -- 'kind("rust_test rule", attr(tags, "manual", //codex-rs/... except //codex-rs/v8-poc/...))'
  )"
fi
if [[ "${RUNNER_OS:-}" != "Windows" ]]; then
  # Non-Windows clippy jobs lint the native test binaries; the
  # Windows-cross binaries exist only for the fast Windows test leg.
  manual_rust_test_targets="$(printf '%s\n' "${manual_rust_test_targets}" | grep -v -- '-windows-cross-bin$' || true)"
elif [[ $windows_cross_compile -eq 1 ]]; then
  # `bazel query` is intentionally pre-analysis and does not remove targets
  # made incompatible by `target_compatible_with`. Sharded integration tests
  # add native-only manual helpers such as `core-all-test-bin`, plus separate
  # `core-all-test-windows-cross-bin` helpers for the Windows cross leg. Keep
  # the Windows helpers and unit-test helpers, but do not pass the native-only
  # sharded integration helpers as explicit clippy targets.
  manual_rust_test_targets="$(printf '%s\n' "${manual_rust_test_targets}" | grep -v -- '-test-bin$' || true)"
fi

printf '%s\n' \
  "//codex-rs/..." \
  "-//codex-rs/v8-poc:all"

# `--config=clippy` on the `workspace_root_test` wrappers does not lint the
# underlying `rust_test` binaries. Add the internal manual `*-unit-tests-bin`
# targets explicitly so inline `#[cfg(test)]` code is linted like
# `cargo clippy --tests`.
printf '%s\n' "${manual_rust_test_targets}"
