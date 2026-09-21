#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if ! "${repo_root}/.github/scripts/run_bazel_with_buildbuddy.py" mod deps --lockfile_mode=error; then
  echo "Unable to verify MODULE.bazel.lock; see the Bazel error above."
  echo "If Bazel reports an out-of-date lockfile, run 'just bazel-lock-update' and commit the updated lockfile."
  exit 1
fi
