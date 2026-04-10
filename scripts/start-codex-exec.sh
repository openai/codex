#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "Usage: $0 HOST [RSYNC_OPTION]..." >&2
}

if [[ $# -lt 1 ]]; then
  usage
  exit 2
fi

case "$1" in
  -h|--help)
    usage
    exit 0
    ;;
esac

remote_host="$1"
shift

remote_path='~/code/codex-sync'
local_exec_server_port="${CODEX_REMOTE_EXEC_SERVER_LOCAL_PORT:-8765}"
remote_exec_server_start_timeout_seconds="${CODEX_REMOTE_EXEC_SERVER_START_TIMEOUT_SECONDS:-15}"

remote_exec_server_pid=''
remote_exec_server_log_path=''
remote_exec_server_pid_path=''
remote_repo_root=''

cleanup() {
  local exit_code=$?

  trap - EXIT INT TERM

  if [[ -n "${remote_exec_server_pid_path}" ]]; then
    ssh "${remote_host}" \
      "if [[ -f '${remote_exec_server_pid_path}' ]]; then kill \$(cat '${remote_exec_server_pid_path}') >/dev/null 2>&1 || true; fi; rm -f '${remote_exec_server_pid_path}' '${remote_exec_server_log_path}'" \
      >/dev/null 2>&1 || true
  fi

  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

if ! command -v ssh >/dev/null 2>&1; then
  echo "ssh is required" >&2
  exit 1
fi

if ! command -v rsync >/dev/null 2>&1; then
  echo "local rsync is required" >&2
  exit 1
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "run this script from inside a git repository" >&2
  exit 1
}

ssh "${remote_host}" "mkdir -p ${remote_path}"

if ! ssh "${remote_host}" 'command -v rsync >/dev/null 2>&1'; then
  echo "remote rsync is required on ${remote_host}" >&2
  exit 1
fi

sync_instance_id="$(date +%s)-$$"

rsync \
  --archive \
  --compress \
  --human-readable \
  --itemize-changes \
  --exclude '.git/' \
  --exclude 'codex-rs/target/' \
  --filter=':- .gitignore' \
  "$@" \
  "${repo_root}/" \
  "${remote_host}:${remote_path}/" \
  >&2

remote_exec_server_log_path="/tmp/codex-exec-server-${sync_instance_id}.log"
remote_exec_server_pid_path="/tmp/codex-exec-server-${sync_instance_id}.pid"

remote_start_output="$(
  ssh "${remote_host}" bash -s -- \
    "${remote_exec_server_log_path}" \
    "${remote_exec_server_pid_path}" \
    "${remote_exec_server_start_timeout_seconds}" <<'EOF'
set -euo pipefail

remote_exec_server_log_path="$1"
remote_exec_server_pid_path="$2"
remote_exec_server_start_timeout_seconds="$3"
remote_repo_root="$HOME/code/codex-sync"
remote_codex_rs="$remote_repo_root/codex-rs"

cd "${remote_codex_rs}"
cargo build -p codex-exec-server --bin codex-exec-server

rm -f "${remote_exec_server_log_path}" "${remote_exec_server_pid_path}"
nohup ./target/debug/codex-exec-server --listen ws://127.0.0.1:0 \
  >"${remote_exec_server_log_path}" 2>&1 &
remote_exec_server_pid="$!"
echo "${remote_exec_server_pid}" >"${remote_exec_server_pid_path}"

deadline=$((SECONDS + remote_exec_server_start_timeout_seconds))
while (( SECONDS < deadline )); do
  if [[ -s "${remote_exec_server_log_path}" ]]; then
    listen_url="$(head -n 1 "${remote_exec_server_log_path}" || true)"
    if [[ "${listen_url}" == ws://* ]]; then
      printf 'remote_exec_server_pid=%s\n' "${remote_exec_server_pid}"
      printf 'remote_exec_server_log_path=%s\n' "${remote_exec_server_log_path}"
      printf 'remote_repo_root=%s\n' "${remote_repo_root}"
      printf 'listen_url=%s\n' "${listen_url}"
      exit 0
    fi
  fi

  if ! kill -0 "${remote_exec_server_pid}" >/dev/null 2>&1; then
    cat "${remote_exec_server_log_path}" >&2 || true
    echo "remote exec server exited before reporting a listen URL" >&2
    exit 1
  fi

  sleep 0.1
done

cat "${remote_exec_server_log_path}" >&2 || true
echo "timed out waiting for remote exec server listen URL" >&2
exit 1
EOF
)"

listen_url=''
while IFS='=' read -r key value; do
  case "${key}" in
    remote_exec_server_pid)
      remote_exec_server_pid="${value}"
      ;;
    remote_exec_server_log_path)
      remote_exec_server_log_path="${value}"
      ;;
    remote_repo_root)
      remote_repo_root="${value}"
      ;;
    listen_url)
      listen_url="${value}"
      ;;
  esac
done <<< "${remote_start_output}"

if [[ -z "${remote_exec_server_pid}" || -z "${listen_url}" || -z "${remote_repo_root}" ]]; then
  echo "failed to parse remote exec server startup output" >&2
  exit 1
fi

remote_exec_server_port="${listen_url##*:}"
if [[ -z "${remote_exec_server_port}" || "${remote_exec_server_port}" == "${listen_url}" ]]; then
  echo "failed to parse remote exec server port from ${listen_url}" >&2
  exit 1
fi

echo "Remote exec server: ${listen_url}"
echo "Remote exec server log: ${remote_exec_server_log_path}"
echo "Press Ctrl-C to stop the SSH tunnel and remote exec server."
echo "Start codex via: "
printf '  CODEX_EXEC_SERVER_URL=ws://127.0.0.1:%s codex -C %q\n' \
  "${local_exec_server_port}" \
  "${remote_repo_root}"

ssh \
  -nNT \
  -o ControlMaster=no \
  -o ControlPath=none \
  -o ExitOnForwardFailure=yes \
  -o ServerAliveInterval=30 \
  -o ServerAliveCountMax=3 \
  -L "${local_exec_server_port}:127.0.0.1:${remote_exec_server_port}" \
  "${remote_host}"
