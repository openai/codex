#!/usr/bin/env bash
set -euo pipefail

# Shim resolver that delegates to the canonical script under codex-rs/scripts.
# Keeps external callers working while centralizing logic in the canonical script.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="${SCRIPT_DIR%/scripts}"
exec "${REPO_ROOT}/codex-rs/scripts/resolve_safe_sync.sh" "$@"
