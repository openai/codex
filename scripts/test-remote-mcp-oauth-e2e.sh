#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/codex-rs/target}"
REMOTE_SERVER_BIN="${REMOTE_SERVER_BIN:-${CARGO_TARGET_DIR}/debug/test_streamable_http_server}"
CODEX_BIN="${CODEX_BIN:-${CARGO_TARGET_DIR}/debug/codex}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codex-remote-mcp-oauth-e2e.XXXXXX")"
CODEX_HOME="${TMP_DIR}/codex-home"
EXEC_SERVER_PORT=""
EXEC_SERVER_LOG_FILE="${TMP_DIR}/exec-server.log"
EXEC_SERVER_PID=""
REMOTE_BOUND_ADDR_FILE="${TMP_DIR}/remote-mcp.addr"
REMOTE_LOG_FILE="${TMP_DIR}/remote-mcp.log"
REMOTE_SERVER_PID=""

cleanup() {
  if [[ -n "${REMOTE_SERVER_PID}" ]]; then
    kill "${REMOTE_SERVER_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${EXEC_SERVER_PID}" ]]; then
    kill "${EXEC_SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

if [[ ! -x "${CODEX_BIN}" || ! -x "${REMOTE_SERVER_BIN}" ]]; then
  (
    cd "${REPO_ROOT}/codex-rs"
    cargo build -p codex-cli --bin codex -p codex-rmcp-client --bin test_streamable_http_server
  )
fi

EXEC_SERVER_PORT="$(
  python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
"${CODEX_BIN}" exec-server --listen "ws://127.0.0.1:${EXEC_SERVER_PORT}" \
  >"${EXEC_SERVER_LOG_FILE}" 2>&1 &
EXEC_SERVER_PID="$!"

deadline=$((SECONDS + 10))
while (( SECONDS < deadline )); do
  if python3 - <<PY >/dev/null 2>&1
import socket

with socket.create_connection(("127.0.0.1", ${EXEC_SERVER_PORT}), timeout=0.2):
    pass
PY
  then
    break
  fi
  sleep 0.05
done
if ! python3 - <<PY >/dev/null 2>&1
import socket

with socket.create_connection(("127.0.0.1", ${EXEC_SERVER_PORT}), timeout=0.2):
    pass
PY
then
  cat "${EXEC_SERVER_LOG_FILE}" >&2 || true
  echo "timed out waiting for exec-server" >&2
  exit 1
fi

MCP_STREAMABLE_HTTP_BIND_ADDR='127.0.0.1:0' \
MCP_STREAMABLE_HTTP_BOUND_ADDR_FILE="${REMOTE_BOUND_ADDR_FILE}" \
"${REMOTE_SERVER_BIN}" >"${REMOTE_LOG_FILE}" 2>&1 &
REMOTE_SERVER_PID="$!"

deadline=$((SECONDS + 10))
REMOTE_BOUND_ADDR=""
while (( SECONDS < deadline )); do
  if REMOTE_BOUND_ADDR="$(cat "${REMOTE_BOUND_ADDR_FILE}" 2>/dev/null)"; then
    break
  fi
  sleep 0.05
done
if [[ -z "${REMOTE_BOUND_ADDR}" ]]; then
  cat "${REMOTE_LOG_FILE}" >&2 || true
  echo "timed out waiting for remote MCP OAuth test server" >&2
  exit 1
fi
REMOTE_PORT="${REMOTE_BOUND_ADDR##*:}"
REMOTE_MCP_URL="http://127.0.0.1:${REMOTE_PORT}/mcp"
CALLBACK_PORT="$(
  python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("0.0.0.0", 0))
    print(sock.getsockname()[1])
PY
)"

mkdir -p "${CODEX_HOME}"
cat > "${CODEX_HOME}/config.toml" <<EOF
model_provider = "mock_provider"
mcp_oauth_credentials_store = "file"
mcp_oauth_callback_port = ${CALLBACK_PORT}
mcp_oauth_callback_url = "http://127.0.0.1:${CALLBACK_PORT}/callback"

[model_providers.mock_provider]
name = "Mock"
base_url = "http://127.0.0.1:1/v1"
wire_api = "responses"

[mcp_servers.remote-oauth]
url = "${REMOTE_MCP_URL}"
environment_id = "remote"

[mcp_servers.remote-oauth.oauth]
client_id = "codex-app-server-test"
EOF
cat > "${CODEX_HOME}/environments.toml" <<EOF
include_local = false

[[environments]]
id = "remote"
url = "ws://127.0.0.1:${EXEC_SERVER_PORT}"
EOF

CODEX_BIN="${CODEX_BIN}" \
CODEX_HOME="${CODEX_HOME}" \
python3 - <<'PY'
import json
import os
import subprocess
import sys
import time
import urllib.request


def send(proc, payload):
    proc.stdin.write(json.dumps(payload) + "\n")
    proc.stdin.flush()


def read_message(proc, timeout_s=15):
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        line = proc.stdout.readline()
        if line:
            return json.loads(line)
        if proc.poll() is not None:
            stderr = proc.stderr.read()
            raise SystemExit(f"codex app-server exited: {stderr}")
        time.sleep(0.01)
    raise SystemExit("timed out waiting for codex app-server message")


def read_until(proc, predicate, timeout_s=15):
    deadline = time.monotonic() + timeout_s
    seen = []
    while time.monotonic() < deadline:
        message = read_message(proc, timeout_s=max(0.1, deadline - time.monotonic()))
        seen.append(message)
        if predicate(message):
            return message
    raise SystemExit(f"timed out waiting for matching message: {seen!r}")


def request(proc, request_id, method, params):
    send(proc, {"id": request_id, "method": method, "params": params})
    message = read_until(proc, lambda message: message.get("id") == request_id)
    if "error" in message:
        raise SystemExit(f"{method} failed: {message['error']}")
    return message["result"]


codex_bin = os.environ["CODEX_BIN"]
codex_home = os.environ["CODEX_HOME"]
proc = subprocess.Popen(
    [codex_bin, "app-server", "--listen", "stdio://"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    env={**os.environ, "CODEX_HOME": codex_home},
)
assert proc.stdin is not None
assert proc.stdout is not None
assert proc.stderr is not None

initialize = request(
    proc,
    1,
    "initialize",
    {
        "clientInfo": {"name": "remote-mcp-oauth-e2e", "version": "0.1.0"},
        "capabilities": {"experimentalApi": True},
    },
)
if "userAgent" not in initialize:
    raise SystemExit(f"initialize response missing userAgent: {initialize!r}")
send(proc, {"method": "initialized"})

status = request(
    proc,
    2,
    "mcpServerStatus/list",
    {"detail": "toolsAndAuthOnly"},
)
entry = status["data"][0]
if entry["name"] != "remote-oauth" or entry["authStatus"] != "notLoggedIn":
    raise SystemExit(f"unexpected pre-login status: {status!r}")

login = request(proc, 3, "mcpServer/oauth/login", {"name": "remote-oauth"})
authorization_url = login["authorizationUrl"]
with urllib.request.urlopen(authorization_url, timeout=15) as response:
    if response.status != 200:
        raise SystemExit(f"unexpected authorize response: {response.status}")

completed = read_until(
    proc,
    lambda message: message.get("method") == "mcpServer/oauthLogin/completed",
)
params = completed.get("params") or {}
if params != {"name": "remote-oauth", "success": True}:
    raise SystemExit(f"unexpected oauth completion notification: {completed!r}")

status = request(
    proc,
    4,
    "mcpServerStatus/list",
    {"detail": "toolsAndAuthOnly"},
)
entry = status["data"][0]
if entry["authStatus"] != "oAuth":
    raise SystemExit(f"unexpected post-login status: {status!r}")

proc.terminate()
proc.wait(timeout=5)
print("remote MCP OAuth E2E passed")
PY
