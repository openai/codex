#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

query_script="${CODEX_BAZEL_QUERY_SCRIPT:-./.github/scripts/run-bazel-query-ci.sh}"

# Enumerate explicit first-party production targets instead of returning a
# recursive package pattern. A recursive build pattern also selects rust_test
# and workspace_root_test rules, which makes build-only cross-compilation
# depend on a target-platform test execution toolchain.
production_targets="$(
  "${query_script}" \
    --output=label \
    -- 'attr(visibility, "//visibility:public", kind("rust_(binary|library|proc_macro) rule", //codex-rs/... except //codex-rs/v8-poc/...))' \
    | LC_ALL=C sort
)"

if [[ -z "${production_targets}" ]]; then
  echo "No Bazel release-build targets found." >&2
  exit 1
fi

printf '%s\n' "${production_targets}"
