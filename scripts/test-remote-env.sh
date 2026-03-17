#!/usr/bin/env bash
set -euo pipefail

# Local harness for codex-rs remote_env integration tests.
# Assumes a working Docker engine (Docker Desktop or Colima).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required (Colima or Docker Desktop)" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable; for Colima run: colima start" >&2
  exit 1
fi

remote_env_dir="$(mktemp -d "${TMPDIR:-/tmp}/codex-remote-env.XXXXXX")"
ssh_key="${remote_env_dir}/id_ed25519"
ssh_key_pub="${ssh_key}.pub"
ssh-keygen -q -t ed25519 -N "" -f "${ssh_key}"
chmod 600 "${ssh_key}"
ssh_pub_key="$(cat "${ssh_key_pub}")"

container_name="codex-remote-test-env-local-$(date +%s)-${RANDOM}"
remote_port="${CODEX_TEST_REMOTE_ENV_LOCAL_PORT:-2222}"

cleanup() {
  docker rm -f "${container_name}" >/dev/null 2>&1 || true
  rm -rf "${remote_env_dir}"
}
trap cleanup EXIT

docker run -d --name "${container_name}" -p "127.0.0.1:${remote_port}:22" ubuntu:24.04 bash -lc "
  set -euo pipefail
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y --no-install-recommends openssh-server
  mkdir -p /run/sshd
  useradd -m -s /bin/bash codex
  install -d -m 700 -o codex -g codex /home/codex/.ssh
  printf '%s\\n' '${ssh_pub_key}' >/home/codex/.ssh/authorized_keys
  chown codex:codex /home/codex/.ssh/authorized_keys
  chmod 600 /home/codex/.ssh/authorized_keys
  exec /usr/sbin/sshd -D -e
"

for attempt in {1..30}; do
  if ssh \
    -i "${ssh_key}" \
    -o BatchMode=yes \
    -o StrictHostKeyChecking=no \
    -o UserKnownHostsFile=/dev/null \
    -o ConnectTimeout=2 \
    -p "${remote_port}" \
    codex@127.0.0.1 \
    true
  then
    break
  fi

  if [[ "${attempt}" -eq 30 ]]; then
    echo "remote env container did not become reachable over ssh" >&2
    docker logs "${container_name}" || true
    exit 1
  fi

  sleep 1
done

export CODEX_TEST_REMOTE_ENV_AVAILABLE=1
export CODEX_TEST_REMOTE_ENV_HOST=127.0.0.1
export CODEX_TEST_REMOTE_ENV_PORT="${remote_port}"
export CODEX_TEST_REMOTE_ENV_USER=codex
export CODEX_TEST_REMOTE_ENV_KEY_PATH="${ssh_key}"

if [[ "$#" -gt 0 ]]; then
  "$@"
else
  (
    cd "${REPO_ROOT}/codex-rs"
    cargo test -p codex-core --test all remote_env_connects_creates_temp_dir_and_runs_sample_script
  )
fi
