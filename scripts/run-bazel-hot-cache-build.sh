#!/usr/bin/env bash

set -euo pipefail

readonly OPENAI_REPOSITORY="openai/codex"
readonly DEFAULT_TARGET="//codex-rs/cli:codex"

usage() {
  cat <<'EOF'
Usage:
  scripts/run-bazel-hot-cache-build.sh [bazel target patterns...]
  scripts/run-bazel-hot-cache-build.sh --print-latest-hot-main-commit

Build the current checkout against the BuildBuddy keyspace warmed by the
platform-matching verify-release-build Bazel CI lane. If no target is
provided, builds //codex-rs/cli:codex.

Requires:
  - macOS or Linux for build mode
  - BUILDBUDDY_API_KEY in the environment for build mode
  - gh auth for --print-latest-hot-main-commit
EOF
}

print_latest_hot_main_commit() {
  if ! command -v gh >/dev/null 2>&1; then
    echo "gh is required to find the latest hot main commit." >&2
    exit 1
  fi

  local hot_commit
  hot_commit="$(
    gh run list \
      --repo "${OPENAI_REPOSITORY}" \
      --workflow Bazel \
      --branch main \
      --event push \
      --limit 20 \
      --json headSha,status,conclusion \
      --jq '[.[] | select(.status == "completed" and .conclusion == "success") | .headSha][0] // empty'
  )"
  if [[ -z "${hot_commit}" ]]; then
    echo "No successful Bazel main push run found." >&2
    exit 1
  fi

  printf '%s\n' "${hot_commit}"
}

case "${1:-}" in
  --help | -h)
    usage
    exit 0
    ;;
  --print-latest-hot-main-commit)
    print_latest_hot_main_commit
    exit 0
    ;;
esac

if [[ -z "${BUILDBUDDY_API_KEY:-}" ]]; then
  echo "BUILDBUDDY_API_KEY must be set to read the OpenAI BuildBuddy cache." >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd "${script_dir}/.." && pwd -P)"
cd "${repo_root}"

targets=("$@")
if [[ ${#targets[@]} -eq 0 ]]; then
  targets=("${DEFAULT_TARGET}")
fi

bazel_bin="${CODEX_BAZEL_BIN:-bazel}"
repository_cache="${BAZEL_REPOSITORY_CACHE:-${HOME}/.cache/bazel-repo-cache}"
commit_sha="${CODEX_BAZEL_COMMIT_SHA:-${GITHUB_SHA:-}}"
if [[ -z "${commit_sha}" ]] && command -v git >/dev/null 2>&1; then
  commit_sha="$(git rev-parse HEAD 2>/dev/null || true)"
fi
if [[ -z "${commit_sha}" ]]; then
  echo "Could not determine COMMIT_SHA; set CODEX_BAZEL_COMMIT_SHA for rsynced mirrors without .git." >&2
  exit 1
fi
bazel_ci_config=""
case "$(uname -s)" in
  Darwin)
    bazel_ci_config="ci-macos"
    ;;
  Linux)
    bazel_ci_config="ci-linux"
    ;;
  *)
    echo "scripts/run-bazel-hot-cache-build.sh supports macOS and Linux only." >&2
    exit 1
    ;;
esac
bazel_startup_args=()
if [[ -n "${BAZEL_OUTPUT_USER_ROOT:-}" ]]; then
  bazel_startup_args+=("--output_user_root=${BAZEL_OUTPUT_USER_ROOT}")
fi

# Keep the explicit Rust debug-assertion flags before the platform CI config.
# That matches the verify-release-build CI action key ordering that warms this
# cache.
exec "${bazel_bin}" \
  "${bazel_startup_args[@]}" \
  --noexperimental_remote_repo_contents_cache \
  build \
  --config=buildbuddy-openai-rbe \
  "--remote_header=x-buildbuddy-api-key=${BUILDBUDDY_API_KEY}" \
  --compilation_mode=fastbuild \
  --@rules_rust//rust/settings:extra_rustc_flag=-Cdebug-assertions=no \
  --@rules_rust//rust/settings:extra_exec_rustc_flag=-Cdebug-assertions=no \
  "--build_metadata=COMMIT_SHA=${commit_sha}" \
  --build_metadata=TAG_job=verify-release-build \
  --build_metadata=TAG_rust_debug_assertions=off \
  "--config=${bazel_ci_config}" \
  --remote_download_toplevel \
  "--repository_cache=${repository_cache}" \
  --noremote_upload_local_results \
  -- \
  "${targets[@]}"
