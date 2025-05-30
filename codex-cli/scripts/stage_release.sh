#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# stage_release.sh
# -----------------------------------------------------------------------------
# Stages an npm release for @openai/codex.
#
# The script used to accept a single optional positional argument that indicated
# the temporary directory in which to stage the package.  We now support a
# flag-based interface so that we can extend the command with further options
# without breaking the call-site contract.
#
#   --tmp <dir>  : Use <dir> instead of a freshly created temp directory.
#   --native     : Bundle the pre-built Rust CLI binaries for Linux alongside
#                  the JavaScript implementation (a so-called "fat" package).
#   -h|--help    : Print usage.
#
# When --native is supplied we copy the linux-sandbox binaries (as before) and
# additionally fetch / unpack the two Rust targets that we currently support:
#   - x86_64-unknown-linux-musl
#   - aarch64-unknown-linux-gnu
#
# NOTE: This script is intended to be run from the repository root via
#       `pnpm --filter codex-cli stage-release ...` or inside codex-cli with the
#       helper script entry in package.json (`pnpm stage-release ...`).
# -----------------------------------------------------------------------------

set -euo pipefail

# Helper - usage / flag parsing

usage() {
  cat <<EOF
Usage: $(basename "$0") [--tmp DIR] [--native]

Options
  --tmp DIR   Use DIR to stage the release (defaults to a fresh mktemp dir)
  --native    Bundle Rust binaries for Linux (fat package)
  -h, --help  Show this help

Legacy positional argument: the first non-flag argument is still interpreted
as the temporary directory (for backwards compatibility) but is deprecated.
EOF
  exit "${1:-0}"
}

TMPDIR=""
INCLUDE_NATIVE=0

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
    --native)
      INCLUDE_NATIVE=1
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

pnpm install
pnpm build

# Paths inside the staged package
mkdir -p "$TMPDIR/bin"

cp -r bin/codex.js "$TMPDIR/bin/codex.js"
cp -r dist "$TMPDIR/dist"
cp -r src "$TMPDIR/src" # keep source for TS sourcemaps
cp ../README.md "$TMPDIR" || true # README is one level up - ignore if missing

# Derive a timestamp-based version (keep same scheme as before)
VERSION="$(printf '0.1.%d' "$(date +%y%m%d%H%M)")"

# Modify package.json - bump version and optionally add the native directory to
# the files array so that the binaries are published to npm.

jq --arg version "$VERSION" \
    '.version = $version' \
    package.json > "$TMPDIR/package.json"

# 2. Native runtime deps (sandbox plus optional Rust binaries)

if [[ "$INCLUDE_NATIVE" -eq 1 ]]; then
  ./scripts/install_native_deps.sh "$TMPDIR" --full-native
  touch "${TMPDIR}/bin/use-native"
else
  ./scripts/install_native_deps.sh "$TMPDIR"
fi

popd >/dev/null

echo "Staged version $VERSION for release in $TMPDIR"

if [[ "$INCLUDE_NATIVE" -eq 1 ]]; then
  echo "Test Rust:"
  echo "    node ${TMPDIR}/bin/codex.js --help"
else
  echo "Test Node:"
  echo "    node ${TMPDIR}/bin/codex.js --help"
fi

# Print final hint for convenience
if [[ "$INCLUDE_NATIVE" -eq 1 ]]; then
  echo "Next:  cd \"$TMPDIR\" && npm publish --tag native"
else
  echo "Next:  cd \"$TMPDIR\" && npm publish"
fi
