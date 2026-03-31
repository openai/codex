#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_root="$repo_root/codex-rs"
listen_url="${CODEX_EXEC_SERVER_LISTEN_URL:-ws://127.0.0.1:0}"
start_timeout_seconds="${CODEX_EXEC_SERVER_START_TIMEOUT_SECONDS:-120}"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/codex-tui-with-exec-server.XXXXXX")"
stdout_log="$tmp_dir/exec-server.stdout"
stderr_log="$tmp_dir/exec-server.stderr"
server_pid=""
exec_server_url=""

cleanup() {
  if [[ -n "$server_pid" ]]; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp_dir"
}

trap cleanup EXIT INT TERM HUP

(
  cd "$cargo_root"
  cargo run -p codex-exec-server -- --listen "$listen_url"
) >"$stdout_log" 2>"$stderr_log" &
server_pid="$!"

# Wait for the server to print its bound websocket URL before launching the TUI.
for _ in $(seq 1 "$((start_timeout_seconds * 20))"); do
  if [[ -s "$stdout_log" ]]; then
    exec_server_url="$(head -n 1 "$stdout_log" | tr -d '\r')"
    if [[ "$exec_server_url" == ws://* ]]; then
      break
    fi
  fi

  if ! kill -0 "$server_pid" >/dev/null 2>&1; then
    cat "$stderr_log" >&2 || true
    cat "$stdout_log" >&2 || true
    echo "failed to start codex-exec-server" >&2
    exit 1
  fi

  sleep 0.05
done

if [[ -z "$exec_server_url" ]]; then
  cat "$stderr_log" >&2 || true
  cat "$stdout_log" >&2 || true
  echo "timed out waiting ${start_timeout_seconds}s for codex-exec-server to report its websocket URL" >&2
  exit 1
fi

export CODEX_EXEC_SERVER_URL="$exec_server_url"
echo "Starting codex-tui with CODEX_EXEC_SERVER_URL=$CODEX_EXEC_SERVER_URL" >&2

cd "$cargo_root"
cargo run -p codex-tui --bin codex-tui -- -c mcp_oauth_credentials_store=file "$@"
