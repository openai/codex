#!/usr/bin/env bash

# Remote-env setup script for codex-rs integration tests.
#
# Usage (source-only):
#   source scripts/test-remote-env.sh
#   cd codex-rs
#   cargo test -p codex-core --test all remote_env_connects_creates_temp_dir_and_runs_sample_script
#   codex_remote_env_cleanup

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

is_sourced() {
  [[ "${BASH_SOURCE[0]}" != "$0" ]]
}

setup_remote_env() {
  local container_name
  local codex_binary_path

  container_name="${CODEX_TEST_REMOTE_ENV_CONTAINER_NAME:-codex-remote-test-env-local-$(date +%s)-${RANDOM}}"
  codex_binary_path="${REPO_ROOT}/codex-rs/target/debug/codex"

  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required (Colima or Docker Desktop)" >&2
    return 1
  fi

  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not reachable; for Colima run: colima start" >&2
    return 1
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to build codex" >&2
    return 1
  fi

  (
    cd "${REPO_ROOT}/codex-rs"
    cargo build -p codex-cli --bin codex
  )

  if [[ ! -f "${codex_binary_path}" ]]; then
    echo "codex binary not found at ${codex_binary_path}" >&2
    return 1
  fi

  docker rm -f "${container_name}" >/dev/null 2>&1 || true
  docker run -d \
    --name "${container_name}" \
    --security-opt seccomp=unconfined \
    ubuntu:24.04 sleep infinity >/dev/null
  if ! docker exec "${container_name}" sh -lc "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y python3 zsh"; then
    docker rm -f "${container_name}" >/dev/null 2>&1 || true
    return 1
  fi

  export CODEX_TEST_REMOTE_ENV="${container_name}"
}

codex_remote_env_cleanup() {
  if [[ -n "${CODEX_TEST_REMOTE_ENV:-}" ]]; then
    docker rm -f "${CODEX_TEST_REMOTE_ENV}" >/dev/null 2>&1 || true
    unset CODEX_TEST_REMOTE_ENV
  fi
}

if ! is_sourced; then
  echo "source this script instead of executing it: source scripts/test-remote-env.sh" >&2
  exit 1
fi

old_shell_options="$(set +o)"
set -euo pipefail
if setup_remote_env; then
  status=0
  echo "CODEX_TEST_REMOTE_ENV=${CODEX_TEST_REMOTE_ENV}"
  echo "Remote env ready. Run your command, then call: codex_remote_env_cleanup"
else
  status=$?
fi
eval "${old_shell_options}"
return "${status}"
