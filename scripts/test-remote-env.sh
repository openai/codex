#!/usr/bin/env bash

# Remote-env test harness for codex-rs integration tests.
#
# Source mode (recommended when you want to prefix cargo test):
#   source scripts/test-remote-env.sh
#   cd codex-rs
#   cargo test -p codex-core --test all remote_env_connects_creates_temp_dir_and_runs_sample_script
#   codex_remote_env_cleanup
#
# Exec mode:
#   ./scripts/test-remote-env.sh
#   ./scripts/test-remote-env.sh bash -lc 'cd codex-rs && cargo test -p codex-core --test all remote_env_connects_creates_temp_dir_and_runs_sample_script'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

is_sourced() {
  [[ "${BASH_SOURCE[0]}" != "$0" ]]
}

ensure_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required (Colima or Docker Desktop)" >&2
    return 1
  fi

  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not reachable; for Colima run: colima start" >&2
    return 1
  fi
}

start_remote_env() {
  local container_name
  container_name="codex-remote-test-env-local-$(date +%s)-${RANDOM}"
  docker run -d --name "${container_name}" ubuntu:24.04 sleep infinity >/dev/null
  export CODEX_TEST_REMOTE_ENV="${container_name}"
}

codex_remote_env_cleanup() {
  if [[ -n "${CODEX_TEST_REMOTE_ENV:-}" ]]; then
    docker rm -f "${CODEX_TEST_REMOTE_ENV}" >/dev/null 2>&1 || true
    unset CODEX_TEST_REMOTE_ENV
  fi
}

run_default_test() {
  (
    cd "${REPO_ROOT}/codex-rs"
    cargo test -p codex-core --test all remote_env_connects_creates_temp_dir_and_runs_sample_script
  )
}

main() {
  ensure_docker
  start_remote_env

  echo "CODEX_TEST_REMOTE_ENV=${CODEX_TEST_REMOTE_ENV}"

  if is_sourced; then
    echo "Remote env ready. Run your command, then call: codex_remote_env_cleanup"
  else
    trap codex_remote_env_cleanup EXIT
    if [[ "$#" -gt 0 ]]; then
      "$@"
    else
      run_default_test
    fi
  fi
}

if is_sourced; then
  old_shell_options="$(set +o)"
  set -euo pipefail
  if main "$@"; then
    status=0
  else
    status=$?
  fi
  eval "${old_shell_options}"
  return "${status}"
else
  set -euo pipefail
  main "$@"
fi
