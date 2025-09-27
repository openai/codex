#!/usr/bin/env bash
set -euo pipefail

sleep 5 & pid=$!
sleep 0.05

# Capability probe
kill -0 "$pid" 2>/dev/null || exit 1

# Send a terminating signal and ensure the child exits
kill -TERM "$pid" || exit 1
wait "$pid" || true
exit 0

