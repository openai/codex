#!/usr/bin/env bash

set -euo pipefail

print_failed_bazel_test_logs=0
use_node_test_env=0
remote_download_toplevel=0
windows_msvc_host_platform=0

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
    --windows-msvc-host-platform)
      windows_msvc_host_platform=1
      shift
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
  echo "Usage: $0 [--print-failed-test-logs] [--use-node-test-env] [--remote-download-toplevel] [--windows-msvc-host-platform] -- <bazel args> -- <targets>" >&2
  exit 1
fi

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

ci_config=ci-linux
case "${RUNNER_OS:-}" in
  macOS)
    ci_config=ci-macos
    ;;
  Windows)
    ci_config=ci-windows
    ;;
esac

print_bazel_test_log_tails() {
  local console_log="$1"
  local testlogs_dir
  local -a bazel_info_cmd=(bazel)

  if (( ${#bazel_startup_args[@]} > 0 )); then
    bazel_info_cmd+=("${bazel_startup_args[@]}")
  fi

  testlogs_dir="$(run_bazel "${bazel_info_cmd[@]:1}" info bazel-testlogs 2>/dev/null || echo bazel-testlogs)"

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

if [[ $use_node_test_env -eq 1 ]]; then
  # Bazel test sandboxes on macOS may resolve an older Homebrew `node`
  # before the `actions/setup-node` runtime on PATH.
  node_bin="$(which node)"
  if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
    node_bin="$(cygpath -w "${node_bin}")"
  fi
  bazel_args+=("--test_env=CODEX_JS_REPL_NODE_PATH=${node_bin}")
fi

post_config_bazel_args=()
if [[ "${RUNNER_OS:-}" == "Windows" && $windows_msvc_host_platform -eq 1 ]]; then
  has_host_platform_override=0
  for arg in "${bazel_args[@]}"; do
    if [[ "$arg" == --host_platform=* ]]; then
      has_host_platform_override=1
      break
    fi
  done

  if [[ $has_host_platform_override -eq 0 ]]; then
    # Keep Windows Bazel targets on `windows-gnullvm` for cfg coverage, but opt
    # specific jobs into an MSVC exec platform when they need helper binaries
    # like Rust test wrappers and V8 generators to resolve a compatible host
    # toolchain.
    post_config_bazel_args+=("--host_platform=//:local_windows_msvc")
  fi
fi

if [[ $remote_download_toplevel -eq 1 ]]; then
  # Override the CI config's remote_download_minimal setting when callers need
  # the built artifact to exist on disk after the command completes.
  post_config_bazel_args+=(--remote_download_toplevel)
fi

if [[ -n "${BAZEL_REPO_CONTENTS_CACHE:-}" ]]; then
  # Windows self-hosted runners can run multiple Bazel jobs concurrently. Give
  # each job its own repo contents cache so they do not fight over the shared
  # path configured in `ci-windows`.
  post_config_bazel_args+=("--repo_contents_cache=${BAZEL_REPO_CONTENTS_CACHE}")
fi

if [[ -n "${BAZEL_REPOSITORY_CACHE:-}" ]]; then
  post_config_bazel_args+=("--repository_cache=${BAZEL_REPOSITORY_CACHE}")
fi

if [[ -n "${CODEX_BAZEL_EXECUTION_LOG_COMPACT_DIR:-}" ]]; then
  post_config_bazel_args+=(
    "--execution_log_compact_file=${CODEX_BAZEL_EXECUTION_LOG_COMPACT_DIR}/execution-log-${bazel_args[0]}-${GITHUB_JOB:-local}-$$.zst"
  )
fi

if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
  windows_action_env_vars=(
    INCLUDE
    LIB
    LIBPATH
    PATH
    UCRTVersion
    UniversalCRTSdkDir
    VCINSTALLDIR
    VCToolsInstallDir
    WindowsLibPath
    WindowsSdkBinPath
    WindowsSdkDir
    WindowsSDKLibVersion
    WindowsSDKVersion
  )

  for env_var in "${windows_action_env_vars[@]}"; do
    if [[ -n "${!env_var:-}" ]]; then
      post_config_bazel_args+=("--action_env=${env_var}" "--host_action_env=${env_var}")
    fi
  done
fi

bazel_console_log="$(mktemp)"
trap 'rm -f "$bazel_console_log"' EXIT

bazel_cmd=(bazel)
if (( ${#bazel_startup_args[@]} > 0 )); then
  bazel_cmd+=("${bazel_startup_args[@]}")
fi

if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
  echo "BuildBuddy API key is available; using remote Bazel configuration."
  # Work around Bazel 9 remote repo contents cache / overlay materialization failures
  # seen in CI (for example "is not a symlink" or permission errors while
  # materializing external repos such as rules_perl). We still use BuildBuddy for
  # remote execution/cache; this only disables the startup-level repo contents cache.
  bazel_run_args=(
    "${bazel_args[@]}"
    "--config=${ci_config}"
    "--remote_header=x-buildbuddy-api-key=${BUILDBUDDY_API_KEY}"
  )
  if (( ${#post_config_bazel_args[@]} > 0 )); then
    bazel_run_args+=("${post_config_bazel_args[@]}")
  fi
  set +e
  run_bazel "${bazel_cmd[@]:1}" \
    --noexperimental_remote_repo_contents_cache \
    "${bazel_run_args[@]}" \
    -- \
    "${bazel_targets[@]}" \
    2>&1 | tee "$bazel_console_log"
  bazel_status=${PIPESTATUS[0]}
  set -e
else
  echo "BuildBuddy API key is not available; using local Bazel configuration."
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
  bazel_run_args=(
    "${bazel_args[@]}"
    --remote_cache=
    --remote_executor=
  )
  if (( ${#post_config_bazel_args[@]} > 0 )); then
    bazel_run_args+=("${post_config_bazel_args[@]}")
  fi
  set +e
  run_bazel "${bazel_cmd[@]:1}" \
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
