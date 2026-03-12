#!/bin/sh
set -eu

require_env() {
  eval "value=\${$1-}"
  if [ -z "$value" ]; then
    echo "missing required env var: $1" >&2
    exit 1
  fi
}

require_env APP_SERVER_URL
require_env APP_SERVER_TEST_CLIENT_BIN

thread_id="${CODEX_THREAD_ID:-${THREAD_ID-}}"
if [ -z "$thread_id" ]; then
  echo "missing required env var: CODEX_THREAD_ID" >&2
  exit 1
fi

hold_seconds="${ELICITATION_HOLD_SECONDS:-15}"
incremented=0

cleanup() {
  if [ "$incremented" -eq 1 ]; then
    "$APP_SERVER_TEST_CLIENT_BIN" --url "$APP_SERVER_URL" \
      thread-decrement-elicitation "$thread_id" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM HUP

echo "[elicitation-hold] increment thread=$thread_id"
"$APP_SERVER_TEST_CLIENT_BIN" --url "$APP_SERVER_URL" \
  thread-increment-elicitation "$thread_id"
incremented=1

echo "[elicitation-hold] sleeping ${hold_seconds}s"
sleep "$hold_seconds"

echo "[elicitation-hold] decrement thread=$thread_id"
"$APP_SERVER_TEST_CLIENT_BIN" --url "$APP_SERVER_URL" \
  thread-decrement-elicitation "$thread_id"
incremented=0

echo "[elicitation-hold] done"
