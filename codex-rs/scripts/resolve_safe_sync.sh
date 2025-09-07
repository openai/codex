#!/usr/bin/env bash
set -euo pipefail

# Resolve SAFE_SYNC and TEST_SCRIPT paths for this repo, preferring codex-rs/scripts over root scripts.
#
# Usage:
#   scripts/resolve_safe_sync.sh [--root <repo_root>] [--emit-gh-env]
#   scripts/resolve_safe_sync.sh [<repo_root>] [--emit-gh-env]   # backward-compatible
#   scripts/resolve_safe_sync.sh --help
#
# Output variables (to stdout):
#   SAFE_SYNC=<path>
#   TEST_SCRIPT=<path>
#   HAS_CODEX_RS=0|1
#   WORKSPACE_PRESENT=0|1
#
# Exit codes:
#   0  success
#   2  scripts not found under <root>/codex-rs/scripts or <root>/scripts
#   3  invalid or unreadable root path

print_help() {
  # Intentionally formatted with single blank-line separators and trailing newline.
  printf '%s\n' 'resolve_safe_sync.sh — choose canonical safe_sync_merge paths'
  printf '\n'
  printf '%s\n' 'Usage:'
  printf '%s\n' '  resolve_safe_sync.sh [--root <repo_root>] [--emit-gh-env]'
  printf '%s\n' '  resolve_safe_sync.sh [<repo_root>] [--emit-gh-env]   # DEPRECATED compat mode; prefer --root'
  printf '%s\n' '  resolve_safe_sync.sh --help'
  printf '\n'
  printf '%s\n' 'Outputs (shell assignments):'
  printf '%s\n' '  SAFE_SYNC, TEST_SCRIPT, HAS_CODEX_RS, WORKSPACE_PRESENT'
  printf '\n'
  printf '%s\n' 'Exit codes:'
  printf '%s\n' '  0 ok • 2 not-found • 3 invalid-root'
}

usage() { print_help; }

ROOT=""
MODE_EMIT_GH_ENV=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      ROOT="${2:-}"
      shift 2
      ;;
    --emit-gh-env)
      MODE_EMIT_GH_ENV=1
      shift
      ;;
    -h|--help)
      usage; exit 0 ;;
    --self-test-help)
      # Verify that help has a trailing newline and no consecutive blank lines.
      tmp=$(mktemp)
      print_help > "$tmp"
      # trailing newline check
      if command -v tail >/dev/null 2>&1 && command -v od >/dev/null 2>&1; then
        last=$(tail -c1 "$tmp" | od -An -t o1 | tr -d ' ')
        if [ "$last" != "012" ]; then
          echo "help missing trailing newline" >&2; rm -f "$tmp"; exit 4
        fi
      else
        # Fallback: append a marker and ensure file ends with newline before marker
        cp "$tmp" "$tmp.chk"
        printf 'X' >> "$tmp.chk"
        sz_orig=$(wc -c < "$tmp")
        sz_chk=$(wc -c < "$tmp.chk")
        if [ $((sz_chk)) -ne $((sz_orig + 1)) ]; then
          echo "help missing trailing newline (fallback)" >&2; rm -f "$tmp" "$tmp.chk"; exit 4
        fi
        rm -f "$tmp.chk"
      fi
      # no consecutive blank lines
      if command -v awk >/dev/null 2>&1; then
        if awk 'blank && /^$/ { exit 5 } { blank=($0=="") } END { exit 0 }' "$tmp"; then
          rm -f "$tmp"; exit 0
        else
          echo "help contains consecutive blank lines" >&2; rm -f "$tmp"; exit 5
        fi
      else
        blank=0
        while IFS= read -r line || [ -n "$line" ]; do
          if [ -z "$line" ]; then
            if [ "$blank" = 1 ]; then echo "help contains consecutive blank lines (fallback)" >&2; rm -f "$tmp"; exit 5; fi
            blank=1
          else
            blank=0
          fi
        done < "$tmp"
        rm -f "$tmp"; exit 0
      fi
      ;;
    *)
      # Back-compat positional root
      if [[ -z "${ROOT}" ]]; then
        ROOT="$1"; shift
      else
        echo "Unknown arg: $1" >&2; usage; exit 3
      fi
      ;;
  esac
done

if [[ -z "${ROOT}" ]]; then
  if ROOT=$(git rev-parse --show-toplevel 2>/dev/null); then :; else ROOT="$PWD"; fi
fi

if [[ ! -d "${ROOT}" ]]; then
  echo "Invalid --root: ${ROOT}" >&2
  exit 3
fi

has_codex_rs=0
safe_sync=""
test_script=""

if [[ -x "${ROOT}/codex-rs/scripts/safe_sync_merge.sh" ]]; then
  safe_sync="${ROOT}/codex-rs/scripts/safe_sync_merge.sh"
  test_script="${ROOT}/codex-rs/scripts/safe_sync_merge_test.sh"
  has_codex_rs=1
elif [[ -x "${ROOT}/scripts/safe_sync_merge.sh" ]]; then
  safe_sync="${ROOT}/scripts/safe_sync_merge.sh"
  test_script="${ROOT}/scripts/safe_sync_merge_test.sh"
else
  echo "safe_sync_merge.sh not found under '${ROOT}/codex-rs/scripts' or '${ROOT}/scripts'" >&2
  exit 2
fi

workspace_present=0
if [[ -f "${ROOT}/Cargo.toml" || -f "${ROOT}/codex-rs/Cargo.toml" ]]; then
  workspace_present=1
fi

if [[ ${MODE_EMIT_GH_ENV} -eq 1 ]]; then
  printf "SAFE_SYNC=%s\n" "${safe_sync}"
  printf "TEST_SCRIPT=%s\n" "${test_script}"
  printf "HAS_CODEX_RS=%s\n" "${has_codex_rs}"
  printf "WORKSPACE_PRESENT=%s\n" "${workspace_present}"
else
  printf "SAFE_SYNC=%q\n" "${safe_sync}"
  printf "TEST_SCRIPT=%q\n" "${test_script}"
  printf "HAS_CODEX_RS=%q\n" "${has_codex_rs}"
  printf "WORKSPACE_PRESENT=%q\n" "${workspace_present}"
fi
