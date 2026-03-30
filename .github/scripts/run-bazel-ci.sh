#!/usr/bin/env bash

set -euo pipefail

print_failed_bazel_test_logs=0
use_node_test_env=0
remote_download_toplevel=0
use_ci_config=0
ci_config_override=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --print-failed-test-logs)
      print_failed_bazel_test_logs=1
      shift
      ;;
    --use-node-test-env)
      use_node_test_env=1
      shift
      ;;
    --remote-download-toplevel)
      remote_download_toplevel=1
      shift
      ;;
    --use-ci-config)
      use_ci_config=1
      shift
      ;;
    --ci-config=*)
      use_ci_config=1
      ci_config_override="${1#*=}"
      shift
      ;;
    --ci-config)
      if [[ $# -lt 2 ]]; then
        echo "Expected a value after --ci-config" >&2
        exit 1
      fi
      use_ci_config=1
      ci_config_override="$2"
      shift 2
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [[ $# -eq 0 ]]; then
  echo "Usage: $0 [--print-failed-test-logs] [--use-node-test-env] [--remote-download-toplevel] [--use-ci-config] [--ci-config=<config>] -- <bazel args> -- <targets>" >&2
  exit 1
fi

bazel_startup_args=()
if [[ -n "${BAZEL_OUTPUT_USER_ROOT:-}" ]]; then
  bazel_startup_args+=("--output_user_root=${BAZEL_OUTPUT_USER_ROOT}")
fi

runner_os="${RUNNER_OS:-}"
if [[ -z "${runner_os}" ]]; then
  case "$(uname -s)" in
    Darwin)
      runner_os=macOS
      ;;
    Linux)
      runner_os=Linux
      ;;
    MINGW*|MSYS*|CYGWIN*)
      runner_os=Windows
      ;;
  esac
fi

ci_config=ci-linux
case "${runner_os}" in
  macOS)
    ci_config=ci-macos
    ;;
  Windows)
    ci_config=ci-windows
    ;;
esac

if [[ -n "${ci_config_override}" ]]; then
  ci_config="${ci_config_override}"
fi

print_bazel_test_log_tails() {
  local console_log="$1"
  local testlogs_dir
  local -a bazel_info_cmd=(bazel)

  if (( ${#bazel_startup_args[@]} > 0 )); then
    bazel_info_cmd+=("${bazel_startup_args[@]}")
  fi

  testlogs_dir="$("${bazel_info_cmd[@]}" info bazel-testlogs 2>/dev/null || echo bazel-testlogs)"

  local failed_targets=()
  while IFS= read -r target; do
    failed_targets+=("$target")
  done < <(
    grep -E '^FAIL: //' "$console_log" \
      | sed -E 's#^FAIL: (//[^ ]+).*#\1#' \
      | sort -u
  )

  if [[ ${#failed_targets[@]} -eq 0 ]]; then
    echo "No failed Bazel test targets were found in console output."
    return
  fi

  for target in "${failed_targets[@]}"; do
    local rel_path="${target#//}"
    rel_path="${rel_path/:/\/}"
    local test_log="${testlogs_dir}/${rel_path}/test.log"

    echo "::group::Bazel test log tail for ${target}"
    if [[ -f "$test_log" ]]; then
      tail -n 200 "$test_log"
    else
      echo "Missing test log: $test_log"
    fi
    echo "::endgroup::"
  done
}

bazel_args=()
bazel_targets=()
found_target_separator=0
for arg in "$@"; do
  if [[ "$arg" == "--" && $found_target_separator -eq 0 ]]; then
    found_target_separator=1
    continue
  fi

  if [[ $found_target_separator -eq 0 ]]; then
    bazel_args+=("$arg")
  else
    bazel_targets+=("$arg")
  fi
done

if [[ ${#bazel_args[@]} -eq 0 || ${#bazel_targets[@]} -eq 0 ]]; then
  echo "Expected Bazel args and targets separated by --" >&2
  exit 1
fi

if [[ $use_node_test_env -eq 1 && "${runner_os}" != "Windows" ]]; then
  # Bazel test sandboxes on macOS may resolve an older Homebrew `node`
  # before the `actions/setup-node` runtime on PATH.
  node_bin="$(which node)"
  bazel_args+=("--test_env=CODEX_JS_REPL_NODE_PATH=${node_bin}")
fi

post_config_bazel_args=()
if [[ $remote_download_toplevel -eq 1 ]]; then
  # Override the CI config's remote_download_minimal setting when callers need
  # the built artifact to exist on disk after the command completes.
  post_config_bazel_args+=(--remote_download_toplevel)
fi

bazel_console_log="$(mktemp)"
trap 'rm -f "$bazel_console_log"' EXIT

bazel_cmd=(bazel)
if (( ${#bazel_startup_args[@]} > 0 )); then
  bazel_cmd+=("${bazel_startup_args[@]}")
fi

ci_config_args=()
if [[ -n "${BUILDBUDDY_API_KEY:-}" || $use_ci_config -eq 1 ]]; then
  ci_config_args+=("--config=${ci_config}")
fi

if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
  echo "BuildBuddy API key is available; using remote Bazel configuration."
  # Work around Bazel 9 remote repo contents cache / overlay materialization failures
  # seen in CI (for example "is not a symlink" or permission errors while
  # materializing external repos such as rules_perl). We still use BuildBuddy for
  # remote execution/cache; this only disables the startup-level repo contents cache.
  bazel_run_args=(
    "${bazel_args[@]}"
    "${ci_config_args[@]}"
    "--remote_header=x-buildbuddy-api-key=${BUILDBUDDY_API_KEY}"
  )
  if (( ${#post_config_bazel_args[@]} > 0 )); then
    bazel_run_args+=("${post_config_bazel_args[@]}")
  fi
  set +e
  "${bazel_cmd[@]}" \
    --noexperimental_remote_repo_contents_cache \
    "${bazel_run_args[@]}" \
    -- \
    "${bazel_targets[@]}" \
    2>&1 | tee "$bazel_console_log"
  bazel_status=${PIPESTATUS[0]}
  set -e
else
  bazel_run_args=("${bazel_args[@]}" "${ci_config_args[@]}")

  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "BuildBuddy API key is not available in GitHub Actions; disabling remote Bazel services."
    # Keep fork/community PRs on Bazel but disable remote services that are
    # configured in .bazelrc and require auth.
    #
    # Flag docs:
    # - Command-line reference: https://bazel.build/reference/command-line-reference
    # - Remote caching overview: https://bazel.build/remote/caching
    # - Remote execution overview: https://bazel.build/remote/rbe
    # - Build Event Protocol overview: https://bazel.build/remote/bep
    #
    # --noexperimental_remote_repo_contents_cache:
    #   disable remote repo contents cache enabled in .bazelrc startup options.
    #   https://bazel.build/reference/command-line-reference#startup_options-flag--experimental_remote_repo_contents_cache
    # --remote_cache= and --remote_executor=:
    #   clear remote cache/execution endpoints configured in .bazelrc.
    #   https://bazel.build/reference/command-line-reference#common_options-flag--remote_cache
    #   https://bazel.build/reference/command-line-reference#common_options-flag--remote_executor
    bazel_run_args+=(
      --remote_cache=
      --remote_executor=
    )
  elif [[ $use_ci_config -eq 1 ]]; then
    echo "BuildBuddy API key env var is not available; using ${ci_config} with remote settings from local Bazel config."
  else
    echo "BuildBuddy API key env var is not available; preserving Bazel remote settings from local Bazel config."
  fi

  if (( ${#post_config_bazel_args[@]} > 0 )); then
    bazel_run_args+=("${post_config_bazel_args[@]}")
  fi
  set +e
  "${bazel_cmd[@]}" \
    --noexperimental_remote_repo_contents_cache \
    "${bazel_run_args[@]}" \
    -- \
    "${bazel_targets[@]}" \
    2>&1 | tee "$bazel_console_log"
  bazel_status=${PIPESTATUS[0]}
  set -e
fi

if [[ ${bazel_status:-0} -ne 0 ]]; then
  if [[ $print_failed_bazel_test_logs -eq 1 ]]; then
    print_bazel_test_log_tails "$bazel_console_log"
  fi
  exit "$bazel_status"
fi
