#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/sanitize-bazel-windows-environment.sh"
sanitize_bazel_windows_environment

print_failed_bazel_test_logs=0
print_failed_bazel_action_summary=0
remote_download_toplevel=0
windows_cross_compile=0
windows_hybrid_execution=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --print-failed-test-logs)
      print_failed_bazel_test_logs=1
      shift
      ;;
    --print-failed-action-summary)
      print_failed_bazel_action_summary=1
      shift
      ;;
    --remote-download-toplevel)
      remote_download_toplevel=1
      shift
      ;;
    --windows-cross-compile)
      windows_cross_compile=1
      shift
      ;;
    --windows-hybrid-execution)
      windows_hybrid_execution=1
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
  echo "Usage: $0 [--print-failed-test-logs] [--print-failed-action-summary] [--remote-download-toplevel] [--windows-cross-compile] [--windows-hybrid-execution] -- <bazel args> -- <targets>" >&2
  exit 1
fi

if [[ $windows_cross_compile -eq 1 && $windows_hybrid_execution -eq 1 ]]; then
  echo "--windows-cross-compile and --windows-hybrid-execution are mutually exclusive" >&2
  exit 1
fi

bazel_startup_args=()
if [[ -n "${BAZEL_OUTPUT_USER_ROOT:-}" ]]; then
  bazel_startup_args+=("--output_user_root=${BAZEL_OUTPUT_USER_ROOT}")
fi

run_bazel() {
  if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
    MSYS2_ARG_CONV_EXCL='*' "$(dirname "${BASH_SOURCE[0]}")/run_bazel_with_buildbuddy.py" "$@"
    return
  fi

  "$(dirname "${BASH_SOURCE[0]}")/run_bazel_with_buildbuddy.py" "$@"
}

run_bazel_with_startup_args() {
  if (( ${#bazel_startup_args[@]} > 0 )); then
    run_bazel "${bazel_startup_args[@]}" "$@"
    return
  fi

  run_bazel "$@"
}

ci_config=ci-linux
case "${RUNNER_OS:-}" in
  macOS)
    ci_config=ci-macos
    ;;
  Windows)
    if [[ $windows_cross_compile -eq 1 ]]; then
      ci_config=ci-windows-cross
    elif [[ $windows_hybrid_execution -eq 1 ]]; then
      ci_config=ci-windows-hybrid
    else
      ci_config=ci-windows
    fi
    ;;
esac

print_bazel_test_log_tails() {
  local console_log="$1"
  local testlogs_dir

  local -a bazel_info_args=(info)
  if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
    # `bazel info` needs the same CI config as the failed test invocation so
    # platform-specific output roots match.
    bazel_info_args+=("--config=${ci_config}")
  fi

  # Only pass flags that affect Bazel's output-root selection or repository
  # lookup. Test/build-only flags such as execution logs or remote download
  # mode can make `bazel info` fail, which would hide the real test log path.
  for arg in "${post_config_bazel_args[@]}"; do
    case "$arg" in
      --host_platform=* | --platforms=* | --repo_contents_cache=* | --repository_cache=*)
        bazel_info_args+=("$arg")
        ;;
    esac
  done

  testlogs_dir="$(run_bazel_with_startup_args \
    --noexperimental_remote_repo_contents_cache \
    "${bazel_info_args[@]}" \
    bazel-testlogs 2>/dev/null || echo bazel-testlogs)"

  local failed_targets=()
  while IFS= read -r target; do
    failed_targets+=("$target")
  done < <(
    grep -E '^(FAIL: //|ERROR: .* Testing //)' "$console_log" \
      | sed -E 's#^FAIL: (//[^ ]+).*#\1#; s#^ERROR: .* Testing (//[^ ]+) failed:.*#\1#' \
      | sort -u
  )

  if [[ ${#failed_targets[@]} -eq 0 ]]; then
    echo "No failed Bazel test targets were found in console output."
    return
  fi

  for target in "${failed_targets[@]}"; do
    local rel_path="${target#//}"
    rel_path="${rel_path/://}"
    local test_log="${testlogs_dir}/${rel_path}/test.log"
    local reported_test_log
    reported_test_log="$(grep -F "FAIL: ${target} " "$console_log" | sed -nE 's#.* \(see (.*[\\/]test\.log)\).*#\1#p' | head -n 1 || true)"
    if [[ -n "$reported_test_log" ]]; then
      reported_test_log="${reported_test_log//\\//}"
      test_log="$reported_test_log"
    fi

    echo "::group::Bazel test log tail for ${target}"
    if [[ -f "$test_log" ]]; then
      tail -n 200 "$test_log"
    else
      echo "Missing test log: $test_log"
    fi
    echo "::endgroup::"
  done
}

print_bazel_action_failure_summary() {
  local console_log="$1"
  local escaped_summary
  local summary

  summary="$(
    awk '
      function clean(line) {
        gsub(sprintf("%c", 27) "\\[[0-9;]*m", "", line)
        sub(/^.*\t[^\t]*\t[0-9TZ:._-]+ /, "", line)
        return line
      }

      function is_diagnostic(line) {
        return line ~ /^(error(\[[^]]+\])?:|warning:|note:|help:)/ ||
          line ~ /^[[:space:]]+-->/ ||
          line ~ /^[[:space:]]*[0-9]+[[:space:]]+\|/ ||
          line ~ /^[[:space:]]*\|/ ||
          line ~ /^[[:space:]]+= (note|help):/ ||
          line ~ /^[[:space:]]*\^[[:space:]^~-]*$/ ||
          line ~ /^For more information/ ||
          line ~ /^error: aborting/
      }

      {
        line = clean($0)
      }

      line ~ /^ERROR: .* failed:/ {
        if (printed) {
          print ""
        }
        print line
        in_failure = 1
        seen_diagnostic = 0
        printed = 1
        next
      }

      in_failure && is_diagnostic(line) {
        print line
        seen_diagnostic = 1
        next
      }

      in_failure && seen_diagnostic && line == "" {
        print ""
        next
      }

      in_failure && seen_diagnostic {
        in_failure = 0
        seen_diagnostic = 0
        next
      }
    ' "$console_log"
  )"

  if [[ -z "$summary" ]]; then
    summary="$(grep -E '^ERROR: |^FAILED: ' "$console_log" | tail -n 50 || true)"
  fi

  if [[ -z "$summary" ]]; then
    echo "No Bazel action failures were found in the captured console output."
    return
  fi

  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    escaped_summary="$(
      printf '%s' "$summary" \
        | awk 'BEGIN { ORS = "" } {
            gsub(/%/, "%25")
            gsub(/\r/, "%0D")
            print sep $0
            sep = "%0A"
          }'
    )"
    echo "::error title=Bazel failed action diagnostics::${escaped_summary}"
  fi

  echo
  echo "Bazel failed action diagnostics:"
  echo "--------------------------------"
  printf '%s\n' "$summary"
  echo "--------------------------------"
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

post_config_bazel_args=()

if [[ $remote_download_toplevel -eq 1 ]]; then
  # Override the CI config's remote_download_minimal setting when callers need
  # the built artifact to exist on disk after the command completes.
  post_config_bazel_args+=(--remote_download_toplevel)
fi

if [[ "${RUNNER_OS:-}" == "Windows" && -n "${BUILDBUDDY_API_KEY:-}" && ( $windows_cross_compile -eq 1 || $windows_hybrid_execution -eq 1 ) ]]; then
  # Bazel derives the default genrule shell from the client host. Remote Linux
  # actions must not be asked to run Git Bash from the Windows runner.
  post_config_bazel_args+=(--shell_executable=/bin/bash)

  if [[ $windows_cross_compile -eq 1 ]]; then
    # `--enable_platform_specific_config` expands `common:windows` on Windows
    # hosts after ordinary rc configs, which can override `ci-windows-cross`'s
    # RBE host platform. Repeat it on the command line for cross builds. The
    # hybrid execution keeps its gnullvm host platform for local Rust actions.
    post_config_bazel_args+=(--host_platform=//:rbe)
  fi
fi

if [[ "${RUNNER_OS:-}" == "Windows" && ! ( -n "${BUILDBUDDY_API_KEY:-}" && ( $windows_cross_compile -eq 1 || "$ci_config" == "ci-windows-argument-lint" ) ) ]]; then
  post_config_bazel_args+=("--shell_executable=${BAZEL_SH}")
fi

if [[ "${RUNNER_OS:-}" == "Windows" && $windows_cross_compile -eq 1 && -z "${BUILDBUDDY_API_KEY:-}" ]]; then
  # The Windows cross-compile config depends on authenticated remote
  # execution. When credentials are unavailable, spell out the equivalent
  # local gnullvm platforms and keep the lower concurrency cap.
  post_config_bazel_args+=(
    --host_platform=//:local_windows
    --platforms=//:windows_x86_64_gnullvm
    --extra_execution_platforms=//:windows_x86_64_gnullvm
    --extra_toolchains=//:windows_gnullvm_tests_on_gnullvm_host_toolchain
    --jobs=8
  )
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
  if [[ -z "${CODEX_BAZEL_WINDOWS_EXECUTION_PATH:-}" ]]; then
    echo "CODEX_BAZEL_WINDOWS_EXECUTION_PATH must be set for Windows Bazel CI." >&2
    exit 1
  fi
  if [[ -z "${CODEX_BAZEL_WINDOWS_TEST_PATH:-}" ]]; then
    echo "CODEX_BAZEL_WINDOWS_TEST_PATH must be set for Windows Bazel CI." >&2
    exit 1
  fi

  windows_execution_path="${CODEX_BAZEL_WINDOWS_EXECUTION_PATH}"
  if [[ -n "${BUILDBUDDY_API_KEY:-}" && ( $windows_cross_compile -eq 1 || $windows_hybrid_execution -eq 1 ) ]]; then
    # Remote build actions run on Linux RBE workers. Give their shell snippets
    # a frozen Linux execution-substrate path. Windows Rust and build-script
    # actions receive the same value, which intentionally cannot discover
    # runner-installed Windows compilers or SDK tools.
    windows_execution_path="/usr/bin:/bin"
  fi
  post_config_bazel_args+=(
    "--action_env=PATH=${windows_execution_path}"
    "--host_action_env=PATH=${windows_execution_path}"
    "--test_env=PATH=${CODEX_BAZEL_WINDOWS_TEST_PATH}"
  )
fi

bazel_console_log="$(mktemp)"
trap 'rm -f "$bazel_console_log"' EXIT

bazel_run_args=(
  "${bazel_args[@]}"
)
if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
  echo "BuildBuddy API key is available; using remote Bazel configuration."
  bazel_run_args+=("--config=${ci_config}")
else
  echo "BuildBuddy API key is not available; using local Bazel configuration."
fi
if (( ${#post_config_bazel_args[@]} > 0 )); then
  bazel_run_args+=("${post_config_bazel_args[@]}")
fi
set +e
# Work around Bazel 9 remote repo contents cache / overlay materialization
# failures seen in CI (for example "is not a symlink" or permission errors
# while materializing external repos such as rules_perl). This only disables
# the startup-level repo contents cache; keyed runs still use BuildBuddy.
run_bazel_with_startup_args \
  --noexperimental_remote_repo_contents_cache \
  "${bazel_run_args[@]}" \
  -- \
  "${bazel_targets[@]}" \
  2>&1 | tee "$bazel_console_log"
bazel_status=${PIPESTATUS[0]}
set -e

if [[ ${bazel_status:-0} -ne 0 ]]; then
  if [[ $print_failed_bazel_action_summary -eq 1 ]]; then
    print_bazel_action_failure_summary "$bazel_console_log"
  fi
  if [[ $print_failed_bazel_test_logs -eq 1 ]]; then
    print_bazel_test_log_tails "$bazel_console_log"
  fi
  exit "$bazel_status"
fi
