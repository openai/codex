#!/usr/bin/env bash

set -euo pipefail

# Run Bazel queries with the same CI startup settings as the main build/test
# invocation so target-discovery queries can reuse the same Bazel server.

query_args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --)
      shift
      break
      ;;
    *)
      query_args+=("$1")
      shift
      ;;
  esac
done

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 [<bazel query args>...] -- <query expression>" >&2
  exit 1
fi

query_expression="$1"

ci_config=ci-linux
case "${RUNNER_OS:-}" in
  macOS)
    ci_config=ci-macos
    ;;
  Windows)
    ci_config=ci-windows
    ;;
esac

bazel_startup_args=()
if [[ -n "${BAZEL_OUTPUT_USER_ROOT:-}" ]]; then
  bazel_startup_args+=("--output_user_root=${BAZEL_OUTPUT_USER_ROOT}")
fi

run_bazel() {
  if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
    MSYS2_ARG_CONV_EXCL='*' bazel "$@"
    return
  fi

  bazel "$@"
}

bazel_query_args=(--noexperimental_remote_repo_contents_cache query)
if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
  bazel_query_args+=(
    "--config=${ci_config}"
    "--remote_header=x-buildbuddy-api-key=${BUILDBUDDY_API_KEY}"
  )
fi

if [[ -n "${BAZEL_REPO_CONTENTS_CACHE:-}" ]]; then
  bazel_query_args+=("--repo_contents_cache=${BAZEL_REPO_CONTENTS_CACHE}")
fi

if [[ -n "${BAZEL_REPOSITORY_CACHE:-}" ]]; then
  bazel_query_args+=("--repository_cache=${BAZEL_REPOSITORY_CACHE}")
fi

bazel_query_args+=("${query_args[@]}" "$query_expression")

if (( ${#bazel_startup_args[@]} > 0 )); then
  run_bazel "${bazel_startup_args[@]}" "${bazel_query_args[@]}"
else
  run_bazel "${bazel_query_args[@]}"
fi
