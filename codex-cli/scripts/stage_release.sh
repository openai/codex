#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# stage_release.sh
# -----------------------------------------------------------------------------
# Stages an npm release for @openai/codex.
#
# Usage:
#
#   --tmp <dir>  : Use <dir> instead of a freshly created temp directory.
#   -h|--help    : Print usage.
#
# -----------------------------------------------------------------------------

set -euo pipefail

# Helper - usage / flag parsing

usage() {
  cat <<EOF
Usage: $(basename "$0") [--tmp DIR] [--version VERSION]

Options
  --tmp DIR   Use DIR to stage the release (defaults to a fresh mktemp dir)
  --version   Specify the version to release (defaults to a timestamp-based version)
  -h, --help  Show this help

Legacy positional argument: the first non-flag argument is still interpreted
as the temporary directory (for backwards compatibility) but is deprecated.
EOF
  exit "${1:-0}"
}

TMPDIR=""
# Default to a timestamp-based version (keep same scheme as before)
VERSION="$(printf '0.1.%d' "$(date +%y%m%d%H%M)")"
WORKFLOW_URL=""

# Manual flag parser - Bash getopts does not handle GNU long options well.
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
      shift || { echo "--workflow-url requires an argument"; exit 1; }
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
      echo "Unexpected extra argument: $1" >&2
      usage 1
      ;;
  esac
  shift
done

# Fallback when the caller did not specify a directory.
# If no directory was specified create a fresh temporary one.
if [[ -z "$TMPDIR" ]]; then
  TMPDIR="$(mktemp -d)"
fi

# Ensure the directory exists, then resolve to an absolute path.
mkdir -p "$TMPDIR"
TMPDIR="$(cd "$TMPDIR" && pwd)"

# Main build logic

echo "Staging release in $TMPDIR"

# The script lives in codex-cli/scripts/ - change into codex-cli root so that
# relative paths keep working.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

pushd "$CODEX_CLI_ROOT" >/dev/null

# 1. Build the JS artifacts ---------------------------------------------------

# Paths inside the staged package
mkdir -p "$TMPDIR/bin"

cp -r bin/codex.js "$TMPDIR/bin/codex.js"
cp ../README.md "$TMPDIR" || true # README is one level up - ignore if missing

# Modify package.json - bump version and optionally add the native directory to
# the files array so that the binaries are published to npm.

jq --arg version "$VERSION" \
    '.version = $version' \
    package.json > "$TMPDIR/package.json"

# 2. Native runtime deps (sandbox plus optional Rust binaries)

./scripts/install_native_deps.sh --workflow-url "$WORKFLOW_URL" "$TMPDIR"

popd >/dev/null

echo "Staged version $VERSION for release in $TMPDIR"

echo "Verify the CLI:"
echo "    node ${TMPDIR}/bin/codex.js --version"
echo "    node ${TMPDIR}/bin/codex.js --help"

# Print final hint for convenience
echo "Next:  cd \"$TMPDIR\" && npm publish"
