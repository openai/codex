#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <target> <binary>" >&2
  exit 2
fi

target="$1"
binary="$2"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Keep the unsigned-executable-memory exception scoped to Intel binaries that link V8.
case "${target}:${binary}" in
  x86_64-apple-darwin:codex | \
    x86_64-apple-darwin:codex-app-server | \
    x86_64-apple-darwin:codex-code-mode-host)
    entitlements="${script_dir}/codex-x86_64-apple-darwin.entitlements.plist"
    ;;
  aarch64-apple-darwin:codex | \
    aarch64-apple-darwin:codex-app-server | \
    aarch64-apple-darwin:codex-code-mode-host | \
    aarch64-apple-darwin:codex-responses-api-proxy | \
    x86_64-apple-darwin:codex-responses-api-proxy)
    entitlements="${script_dir}/codex.entitlements.plist"
    ;;
  *)
    echo "unsupported macOS signing target and binary: ${target} ${binary}" >&2
    exit 2
    ;;
esac

printf '%s\n' "$entitlements"
