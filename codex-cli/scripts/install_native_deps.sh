#!/bin/bash

# Copy the Linux sandbox native binaries into the bin/ subfolder of codex-cli/.
#
# Usage:
#   ./scripts/install_native_deps.sh [CODEX_CLI_ROOT]
#
# Arguments
#   [CODEX_CLI_ROOT] – Optional.  If supplied, it should be the codex-cli
#                      folder that contains the package.json for @openai/codex.
#
# When no argument is given we assume the script is being run directly from a
# development checkout. In that case we install the binaries into the
# repository’s own `bin/` directory so that the CLI can run locally.

set -euo pipefail

# ----------------------------------------------------------------------------
# Determine where the binaries should be installed.
# ----------------------------------------------------------------------------

if [[ $# -gt 0 ]]; then
  # The caller supplied a release root directory.
  CODEX_CLI_ROOT="$1"
  BIN_DIR="$CODEX_CLI_ROOT/bin"
else
  # No argument; fall back to the repo’s own bin directory.
  # Resolve the path of this script, then walk up to the repo root.
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  BIN_DIR="$CODEX_CLI_ROOT/bin"
fi

# Make sure the destination directory exists.
mkdir -p "$BIN_DIR"

# ----------------------------------------------------------------------------
# Download and decompress the artifacts from the GitHub Actions workflow.
# ----------------------------------------------------------------------------

# Until we start publishing stable GitHub releases, we have to grab the binaries
# from the GitHub Action that created them. Update the URL below to point to the
# appropriate workflow run:
WORKFLOW_URL="https://github.com/openai/codex/actions/runs/14763725716"
WORKFLOW_ID="${WORKFLOW_URL##*/}"

ARTIFACTS_DIR="$(mktemp -d)"
trap 'rm -rf "$ARTIFACTS_DIR"' EXIT

# NB: The GitHub CLI `gh` must be installed and authenticated.
gh run download --dir "$ARTIFACTS_DIR" --repo openai/codex "$WORKFLOW_ID"

# Decompress the two target architectures.
zstd -d "$ARTIFACTS_DIR/x86_64-unknown-linux-musl/codex-linux-sandbox-x86_64-unknown-linux-musl.zst" \
     -o "$BIN_DIR/codex-linux-sandbox-x64"

zstd -d "$ARTIFACTS_DIR/aarch64-unknown-linux-gnu/codex-linux-sandbox-aarch64-unknown-linux-gnu.zst" \
     -o "$BIN_DIR/codex-linux-sandbox-arm64"

echo "Installed native dependencies into $BIN_DIR"

