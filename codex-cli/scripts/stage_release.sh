#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# stage_release.sh
# -----------------------------------------------------------------------------
# Stages an npm release for @openai/codex.
#
# Usage:
#
#   --tmp <dir>  : Use <dir> instead of a freshly created temp directory.
#   --version    : Version string to write into package.json.
#   --workflow-url <url>: Workflow run that produced the native binaries.
#   -h|--help    : Print usage.
#
# -----------------------------------------------------------------------------

set -euo pipefail

usage() {
  cat <<EOF
Usage: $(basename "$0") [--tmp DIR] [--version VERSION] [--workflow-url URL]

Options
  --tmp DIR         Use DIR to stage the release (defaults to a fresh mktemp dir)
  --version VERSION Set the npm package version (defaults to timestamp scheme)
  --workflow-url URL  Workflow run URL that produced the native binaries
  -h, --help        Show this help

Legacy positional argument: the first non-flag argument is still interpreted
as the temporary directory (for backwards compatibility) but is deprecated.
EOF
  exit "${1:-0}"
}

TMPDIR=""
VERSION="$(printf '0.1.%d' "$(date +%y%m%d%H%M)")"
WORKFLOW_URL=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tmp)
      shift || { echo "--tmp requires an argument"; usage 1; }
      TMPDIR="$1"
      ;;
    --tmp=*)
      TMPDIR="${1#*=}"
      ;;
    --version)
      shift || { echo "--version requires an argument"; usage 1; }
      VERSION="$1"
      ;;
    --workflow-url)
      shift || { echo "--workflow-url requires an argument"; usage 1; }
      WORKFLOW_URL="$1"
      ;;
    -h|--help)
      usage 0
      ;;
    --*)
      echo "Unknown option: $1" >&2
      usage 1
      ;;
    *)
      if [[ -z "$TMPDIR" ]]; then
        TMPDIR="$1"
      else
        echo "Unexpected extra argument: $1" >&2
        usage 1
      fi
      ;;
  esac
  shift
done

if [[ -z "$TMPDIR" ]]; then
  TMPDIR="$(mktemp -d)"
fi

mkdir -p "$TMPDIR"
TMPDIR="$(cd "$TMPDIR" && pwd)"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BUILD_ARGS=(
  --version "$VERSION"
  --staging-dir "$TMPDIR"
)
if [[ -n "$WORKFLOW_URL" ]]; then
  BUILD_ARGS+=(--workflow-url "$WORKFLOW_URL")
fi

python3 "$CODEX_CLI_ROOT/scripts/build_npm_package.py" "${BUILD_ARGS[@]}"

cat <<EOF
Staged version $VERSION for release in $TMPDIR

Verify the CLI:
    node ${TMPDIR}/bin/codex.js --version
    node ${TMPDIR}/bin/codex.js --help

Next:  cd "$TMPDIR" && npm publish
EOF
