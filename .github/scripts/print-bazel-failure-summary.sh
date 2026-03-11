#!/usr/bin/env bash
set -euo pipefail

console_log="${1:?usage: print-bazel-failure-summary.sh <console-log> [testlogs-dir]}"
testlogs_dir="${2:-${BAZEL_TESTLOGS_DIR:-}}"

print_interesting_test_log_lines() {
  local test_log="$1"
  local failure_pattern='panicked at|assertion .* failed|snapshot assertion|^failures:$|^failures:|^---- .* stdout ----$|^thread '\''.*'\'' panicked|^Error: |^Caused by:|^test result: FAILED'

  grep -nE -C 3 "$failure_pattern" "$test_log" | tail -n 120
}

print_interesting_console_failure_lines() {
  local log_file="$1"
  local failure_pattern='^ERROR: |^FAILED:|^FAIL: |^error: |^Caused by:|panicked at|assertion .* failed|^failures:$|^failures:|^test result: FAILED'

  grep -nE -C 2 "$failure_pattern" "$log_file" | tail -n 160
}

print_buildbuddy_link() {
  local log_file="$1"

  grep -m 1 'Streaming build results to:' "$log_file"
}

print_failed_bazel_test_logs() {
  local log_file="$1"
  local resolved_testlogs_dir="$2"

  local failed_targets=()
  while IFS= read -r target; do
    failed_targets+=("$target")
  done < <(
    grep -E '^FAIL: //' "$log_file" \
      | sed -E 's#^FAIL: (//[^ ]+).*#\1#' \
      | sort -u
  )

  if [[ ${#failed_targets[@]} -eq 0 ]]; then
    echo "::group::Bazel failure summary"
    if ! print_interesting_console_failure_lines "$log_file"; then
      echo "No focused failure lines matched; showing console tail instead."
      tail -n 120 "$log_file"
    fi
    echo "::endgroup::"
    return
  fi

  for target in "${failed_targets[@]}"; do
    local rel_path="${target#//}"
    rel_path="${rel_path/://}"
    local test_log="${resolved_testlogs_dir}/${rel_path}/test.log"

    echo "::group::Bazel test summary for ${target}"
    if [[ -f "$test_log" ]]; then
      if ! print_interesting_test_log_lines "$test_log"; then
        echo "No focused failure lines matched; showing tail instead."
        tail -n 120 "$test_log"
      fi
    else
      echo "Missing test log: $test_log"
    fi
    echo "::endgroup::"
  done
}

if ! print_buildbuddy_link "$console_log"; then
  echo "BuildBuddy invocation link was not found in Bazel output."
fi

if [[ -z "$testlogs_dir" ]]; then
  print_failed_bazel_test_logs "$console_log" ""
  exit 0
fi

print_failed_bazel_test_logs "$console_log" "$testlogs_dir"
