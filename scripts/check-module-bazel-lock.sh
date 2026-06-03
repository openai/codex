#!/usr/bin/env bash
set -euo pipefail

use_ci_bazel=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ci)
      use_ci_bazel=1
      shift
      ;;
    *)
      echo "Usage: $0 [--ci]" >&2
      exit 1
      ;;
  esac
done

bazel_status=0
if [[ $use_ci_bazel -eq 1 ]]; then
  bazel_startup_args=(--noexperimental_remote_repo_contents_cache)
  if [[ -n "${BAZEL_OUTPUT_USER_ROOT:-}" ]]; then
    bazel_startup_args=(
      "--output_user_root=${BAZEL_OUTPUT_USER_ROOT}"
      "${bazel_startup_args[@]}"
    )
  fi
  bazel "${bazel_startup_args[@]}" mod deps --lockfile_mode=error || bazel_status=$?
else
  bazel mod deps --lockfile_mode=error || bazel_status=$?
fi

if [[ $bazel_status -ne 0 ]]; then
  echo "MODULE.bazel.lock is out of date."
  echo "Run 'just bazel-lock-update' and commit the updated lockfile."
  exit 1
fi
