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
#   CODEX_LINUX_SANDBOX_NO_PROC=1    # default: 1 (pass --no-proc for bwrap suite)
#   CODEX_LINUX_SANDBOX_DEBUG=1      # default: 0 (pass debug env var through)
#   CODEX_LINUX_SANDBOX_USE_BWRAP=1  # default: 1 (run the bwrap suite)
#   CODEX_LINUX_SANDBOX_USE_LEGACY=1 # default: 1 (run the legacy suite)
#   CODEX_LINUX_SANDBOX_USE_VENDORED=1 # default: 0 (use build-time bwrap FFI)
#   CODEX_LINUX_SANDBOX_BWRAP_PATH   # default: $(command -v bwrap)

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script is intended to run on Linux." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
CODEX_RS_DIR="${REPO_ROOT}/codex-rs"

NO_PROC="${CODEX_LINUX_SANDBOX_NO_PROC:-1}"
DEBUG="${CODEX_LINUX_SANDBOX_DEBUG:-0}"
USE_BWRAP_SUITE="${CODEX_LINUX_SANDBOX_USE_BWRAP:-1}"
USE_LEGACY_SUITE="${CODEX_LINUX_SANDBOX_USE_LEGACY:-1}"
USE_VENDORED="${CODEX_LINUX_SANDBOX_USE_VENDORED:-0}"

BWRAP_PATH=""
if [[ "${USE_VENDORED}" != "1" ]]; then
  BWRAP_PATH="${CODEX_LINUX_SANDBOX_BWRAP_PATH:-$(command -v bwrap || true)}"
  if [[ -z "${BWRAP_PATH}" ]]; then
    echo "bubblewrap (bwrap) is required but was not found on PATH." >&2
    exit 1
  fi
fi

SANDBOX_BIN="${CODEX_RS_DIR}/target/debug/codex-linux-sandbox"
tmp_root=""

build_binary() {
  echo "==> Building codex-linux-sandbox"
  (cd "${CODEX_RS_DIR}" && cargo build -p codex-linux-sandbox >/dev/null)
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
    if [[ "${USE_VENDORED}" == "1" ]]; then
      bwrap_flag=(--use-vendored-bwrap)
    else
      bwrap_flag=(--bwrap-path "${BWRAP_PATH}")
    fi
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

main() {
  build_binary

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
