#!/usr/bin/env bash
set -euo pipefail

resolve_runfile() {
  local logical_path="$1"
  local workspace_logical_path="${logical_path}"
  if [[ -n "${TEST_WORKSPACE:-}" ]]; then
    workspace_logical_path="${TEST_WORKSPACE}/${logical_path}"
  fi

  for runfiles_root in "${RUNFILES_DIR:-}" "${TEST_SRCDIR:-}"; do
    if [[ -n "${runfiles_root}" && -e "${runfiles_root}/${logical_path}" ]]; then
      printf '%s\n' "${runfiles_root}/${logical_path}"
      return 0
    fi
    if [[ -n "${runfiles_root}" && -e "${runfiles_root}/${workspace_logical_path}" ]]; then
      printf '%s\n' "${runfiles_root}/${workspace_logical_path}"
      return 0
    fi
  done

  local manifest="${RUNFILES_MANIFEST_FILE:-}"
  if [[ -z "${manifest}" ]]; then
    if [[ -f "$0.runfiles_manifest" ]]; then
      manifest="$0.runfiles_manifest"
    elif [[ -f "$0.exe.runfiles_manifest" ]]; then
      manifest="$0.exe.runfiles_manifest"
    fi
  fi

  if [[ -n "${manifest}" && -f "${manifest}" ]]; then
    local resolved=""
    resolved="$(awk -v key="${logical_path}" '$1 == key { $1 = ""; sub(/^ /, ""); print; exit }' "${manifest}")"
    if [[ -z "${resolved}" ]]; then
      resolved="$(awk -v key="${workspace_logical_path}" '$1 == key { $1 = ""; sub(/^ /, ""); print; exit }' "${manifest}")"
    fi
    if [[ -n "${resolved}" ]]; then
      printf '%s\n' "${resolved}"
      return 0
    fi
  fi

  echo "failed to resolve runfile: $logical_path" >&2
  return 1
}

workspace_root_marker="$(resolve_runfile "__WORKSPACE_ROOT_MARKER__")"
workspace_root="$(dirname "$(dirname "$(dirname "${workspace_root_marker}")")")"
test_bin="$(resolve_runfile "__TEST_BIN__")"

test_shard_index() {
  local test_name="$1"
  # FNV-1a 32-bit hash. Keep this stable so adding one test does not reshuffle
  # unrelated tests between shards.
  local hash=2166136261
  local byte
  local char
  local i
  local LC_ALL=C

  for ((i = 0; i < ${#test_name}; i++)); do
    char="${test_name:i:1}"
    printf -v byte "%d" "'$char"
    hash=$(( ((hash ^ byte) * 16777619) & 0xffffffff ))
  done

  echo $(( hash % TOTAL_SHARDS ))
}

run_sharded_libtest() {
  if [[ -n "${TEST_SHARD_STATUS_FILE:-}" && "${TEST_TOTAL_SHARDS:-0}" != "0" ]]; then
    touch "${TEST_SHARD_STATUS_FILE}"
  fi

  # Extra libtest args are usually ad-hoc local filters. Preserve those exactly
  # rather than combining them with generated exact filters.
  if [[ $# -gt 0 ]]; then
    exec "${test_bin}" "$@"
  fi

  if [[ -z "${SHARD_INDEX}" ]]; then
    echo "TEST_SHARD_INDEX or RULES_RUST_TEST_SHARD_INDEX must be set when sharding is enabled" >&2
    exit 1
  fi

  local list_output
  local test_list
  list_output="$("${test_bin}" --list --format terse)"
  test_list="$(printf '%s\n' "${list_output}" | grep ': test$' | sed 's/: test$//' | LC_ALL=C sort || true)"

  if [[ -z "${test_list}" ]]; then
    exit 0
  fi

  local shard_tests=()
  local test_name
  while IFS= read -r test_name; do
    if (( $(test_shard_index "${test_name}") == SHARD_INDEX )); then
      shard_tests+=("${test_name}")
    fi
  done <<< "${test_list}"

  if [[ ${#shard_tests[@]} -eq 0 ]]; then
    exit 0
  fi

  exec "${test_bin}" "${shard_tests[@]}" --exact
}

export INSTA_WORKSPACE_ROOT="${workspace_root}"
cd "${workspace_root}"

TOTAL_SHARDS="${RULES_RUST_TEST_TOTAL_SHARDS:-${TEST_TOTAL_SHARDS:-}}"
SHARD_INDEX="${RULES_RUST_TEST_SHARD_INDEX:-${TEST_SHARD_INDEX:-}}"
if [[ -n "${TOTAL_SHARDS}" && "${TOTAL_SHARDS}" != "0" ]]; then
  run_sharded_libtest "$@"
fi

exec "${test_bin}" "$@"
