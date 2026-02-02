#!/usr/bin/env bash
set -euo pipefail

# Linux sandbox smoke test script.
#
# This is designed for Linux devboxes where bwrap is available. It builds the
# codex-linux-sandbox binary and runs a small matrix of behavior checks:
# - workspace writes succeed
# - protected paths (.git, .codex) remain read-only
# - writes outside allowed roots fail
# - network_access=false blocks outbound sockets
#
# Usage:
#   codex-rs/linux-sandbox/scripts/test_linux_sandbox.sh
#
# Optional env vars:
#   CODEX_BWRAP_ENABLE_FFI=1         # default: 1 (build vendored bwrap path)
#   CODEX_LINUX_SANDBOX_NO_PROC=0    # default: 0 (let helper auto-retry with --no-proc)
#   CODEX_LINUX_SANDBOX_DEBUG=1      # default: 0 (pass debug env var through)
#   CODEX_LINUX_SANDBOX_USE_BWRAP=1  # default: 1 (run the bwrap suite)
#   CODEX_LINUX_SANDBOX_USE_LEGACY=1 # default: 0 (run the legacy suite)
#   CODEX_LINUX_SANDBOX_USE_CODEX_CLI=1 # default: 1 (run codex CLI bwrap smoke)

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script is intended to run on Linux." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
CODEX_RS_DIR="${REPO_ROOT}/codex-rs"

BWRAP_ENABLE_FFI="${CODEX_BWRAP_ENABLE_FFI:-1}"
NO_PROC="${CODEX_LINUX_SANDBOX_NO_PROC:-0}"
DEBUG="${CODEX_LINUX_SANDBOX_DEBUG:-0}"
USE_BWRAP_SUITE="${CODEX_LINUX_SANDBOX_USE_BWRAP:-1}"
USE_LEGACY_SUITE="${CODEX_LINUX_SANDBOX_USE_LEGACY:-0}"
USE_CODEX_CLI_SMOKE="${CODEX_LINUX_SANDBOX_USE_CODEX_CLI:-1}"

SANDBOX_BIN="${CODEX_RS_DIR}/target/debug/codex-linux-sandbox"
CODEX_BIN="${CODEX_RS_DIR}/target/debug/codex"
tmp_root=""

build_binaries() {
  echo "==> Building codex-linux-sandbox"
  (
    cd "${CODEX_RS_DIR}" && \
      CODEX_BWRAP_ENABLE_FFI="${BWRAP_ENABLE_FFI}" cargo build -p codex-linux-sandbox >/dev/null
  )

  if [[ "${USE_CODEX_CLI_SMOKE}" == "1" ]]; then
    echo "==> Building codex (local target/debug/codex)"
    (
      cd "${CODEX_RS_DIR}" && \
        CODEX_BWRAP_ENABLE_FFI="${BWRAP_ENABLE_FFI}" cargo build -p codex-cli >/dev/null
    )
  fi
}

policy_json() {
  local network_access="$1"
  printf '{"type":"workspace-write","writable_roots":[],"network_access":%s}' "${network_access}"
}

run_sandbox() {
  local network_access="$1"
  local use_bwrap="$2"
  shift
  shift

  local no_proc_flag=()
  if [[ "${NO_PROC}" == "1" && "${use_bwrap}" == "1" ]]; then
    no_proc_flag=(--no-proc)
  fi

  local debug_env=()
  if [[ "${DEBUG}" == "1" ]]; then
    debug_env=(env CODEX_LINUX_SANDBOX_DEBUG=1)
  fi

  local bwrap_flag=()
  if [[ "${use_bwrap}" == "1" ]]; then
    bwrap_flag=(--use-bwrap-sandbox --use-vendored-bwrap)
  fi

  "${debug_env[@]}" "${SANDBOX_BIN}" \
    --sandbox-policy-cwd "${REPO_ROOT}" \
    --sandbox-policy "$(policy_json "${network_access}")" \
    "${bwrap_flag[@]}" \
    "${no_proc_flag[@]}" \
    -- "$@"
}

expect_success() {
  local label="$1"
  local network_access="$2"
  local use_bwrap="$3"
  shift
  shift
  shift
  echo "==> ${label}"
  if run_sandbox "${network_access}" "${use_bwrap}" "$@"; then
    echo "    PASS"
  else
    echo "    FAIL (expected success)" >&2
    exit 1
  fi
}

expect_failure() {
  local label="$1"
  local network_access="$2"
  local use_bwrap="$3"
  shift
  shift
  shift
  echo "==> ${label}"
  if run_sandbox "${network_access}" "${use_bwrap}" "$@"; then
    echo "    FAIL (expected failure)" >&2
    exit 1
  else
    echo "    PASS (failed as expected)"
  fi
}

run_suite() {
  local suite_name="$1"
  local use_bwrap="$2"

  echo
  echo "==== Suite: ${suite_name} (use_bwrap=${use_bwrap}) ===="

  # Create a disposable writable root for workspace-write checks.
  if [[ -n "${tmp_root:-}" ]]; then
    rm -rf -- "${tmp_root}"
  fi
  tmp_root="$(mktemp -d "${REPO_ROOT}/.codex-sandbox-test.XXXXXX")"
  trap 'rm -rf -- "${tmp_root:-}"' EXIT

  mkdir -p "${REPO_ROOT}/.codex"

  expect_success \
    "workspace write succeeds inside repo" \
    true \
    "${use_bwrap}" \
    /usr/bin/bash -lc "cd '${tmp_root}' && touch OK_IN_WORKSPACE"

  expect_failure \
    "writes outside allowed roots fail" \
    true \
    "${use_bwrap}" \
    /usr/bin/bash -lc "touch /etc/SHOULD_FAIL"

  # Only the bwrap suite enforces `.git` and `.codex` as read-only.
  if [[ "${use_bwrap}" == "1" ]]; then
    expect_failure \
      ".git and .codex remain read-only (bwrap)" \
      true \
      "${use_bwrap}" \
      /usr/bin/bash -lc "cd '${REPO_ROOT}' && touch .git/SHOULD_FAIL && touch .codex/SHOULD_FAIL"
  else
    expect_success \
      ".git and .codex are NOT protected in legacy landlock path" \
      true \
      "${use_bwrap}" \
      /usr/bin/bash -lc "cd '${REPO_ROOT}' && mkdir -p .codex && touch .git/SHOULD_SUCCEED && touch .codex/SHOULD_SUCCEED"
  fi

  expect_failure \
    "network_access=false blocks outbound sockets" \
    false \
    "${use_bwrap}" \
    /usr/bin/bash -lc "exec 3<>/dev/tcp/1.1.1.1/443"
}

run_codex_cli_smoke() {
  if [[ "${USE_CODEX_CLI_SMOKE}" != "1" ]]; then
    return
  fi

  echo
  echo "==== codex CLI smoke (feature flag path) ===="
  echo "==> codex -c features.use_linux_sandbox_bwrap=true sandbox linux ..."

  local output=""
  if ! output=$(
    "${CODEX_BIN}" \
      -c features.use_linux_sandbox_bwrap=true \
      sandbox linux --full-auto -- /usr/bin/bash -lc 'echo BWRAP_OK' 2>&1
  ); then
    echo "${output}" >&2
    echo "    FAIL (expected codex CLI bwrap smoke success)" >&2
    exit 1
  fi

  if [[ "${output}" != *"BWRAP_OK"* ]]; then
    echo "${output}" >&2
    echo "    FAIL (missing BWRAP_OK output)" >&2
    exit 1
  fi

  echo "${output}"
  echo "    PASS"
}

main() {
  build_binaries
  run_codex_cli_smoke

  if [[ "${USE_BWRAP_SUITE}" == "1" ]]; then
    run_suite "bwrap opt-in" "1"
  fi

  if [[ "${USE_LEGACY_SUITE}" == "1" ]]; then
    run_suite "legacy default" "0"
  fi

  echo
  echo "All requested linux-sandbox suites passed."
}

main "$@"
