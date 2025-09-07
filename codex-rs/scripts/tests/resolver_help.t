#!/usr/bin/env bash
set -euo pipefail

# Minimal portable test: regenerate normalized resolver help and diff with golden.

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

export LC_ALL=C LANG=C
HELP_OUT=$(bash scripts/resolve_safe_sync.sh --help)

normalize() {
  if command -v tr >/dev/null 2>&1 && command -v awk >/dev/null 2>&1; then
    tr -d '\r' | awk 'NF{print $0}' ORS='\n'
  else
    # sed-only fallback: strip CR and trailing blank lines
    sed -e 's/\r$//' -e :a -e '/^[[:space:]]*$/{$d;N;ba' -e '}'
  fi
}

TMP=$(mktemp)
printf '%s\n' "$HELP_OUT" | normalize > "$TMP"

GOLDEN="codex-rs/docs/golden/resolver_help.txt"
diff -u "$GOLDEN" "$TMP"
rm -f "$TMP"
echo "[ok] resolver_help_golden"
