#!/usr/bin/env bash

set -euo pipefail

manual_rust_test_targets=()
# Wildcard target patterns skip manual-tagged unit-test binaries, so resolve
# them explicitly to keep Bazel lint coverage aligned with Cargo --all-targets.
while IFS= read -r label; do
  manual_rust_test_targets+=("$label")
done < <(bazel query 'kind("rust_test rule", attr(tags, manual, //codex-rs/...))')

./.github/scripts/run-bazel-ci.sh \
  -- \
  build \
  "$@" \
  -- \
  //codex-rs/... \
  "${manual_rust_test_targets[@]}"
