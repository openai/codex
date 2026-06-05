#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

query_script="${CODEX_BAZEL_QUERY_SCRIPT:-./.github/scripts/run-bazel-query-ci.sh}"

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
# continue with a partial list if `bazel query` fails. Target discovery is
# local on all platforms.
production_targets="$(
  "${query_script}" \
    --output=label \
    -- 'attr(visibility, "//visibility:public", kind("rust_(binary|library|proc_macro) rule", //codex-rs/... except //codex-rs/v8-poc/...))' \
    | LC_ALL=C sort
)"

if [[ -z "${production_targets}" ]]; then
  echo "No Bazel clippy production targets found." >&2
  exit 1
fi

if [[ $windows_cross_compile -eq 1 ]]; then
  # Build-only cross-compilation has no target-platform test runner. Lint the
  # complete production surface without analyzing rust_test or
  # workspace_root_test rules.
  printf '%s\n' "${production_targets}"
  exit 0
fi

manual_rust_test_targets="$(
  "${query_script}" \
    --output=label \
    -- 'kind("rust_test rule", attr(tags, "manual", //codex-rs/... except //codex-rs/v8-poc/...))'
)"
manual_rust_test_targets="$(
  printf '%s\n' "${manual_rust_test_targets}" \
    | grep -v -- '-windows-cross-bin$' \
    | LC_ALL=C sort \
    || true
)"

# `--config=clippy` on the `workspace_root_test` wrappers does not lint the
# underlying `rust_test` binaries. Add the internal manual `*-unit-tests-bin`
# targets explicitly so inline `#[cfg(test)]` code is linted like
# `cargo clippy --tests`.
printf '%s\n' "${production_targets}"
if [[ -n "${manual_rust_test_targets}" ]]; then
  printf '%s\n' "${manual_rust_test_targets}"
fi
