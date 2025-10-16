#!/usr/bin/env bash
set -euo pipefail
log_file="/tmp/codex-notifications.log"
printf '%s
' "$*" >> "$log_file"
