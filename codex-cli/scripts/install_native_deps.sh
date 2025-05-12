#!/usr/bin/env bash

# Install native runtime dependencies for codex-cli.
#
# By default the script copies the sandbox binaries that are required at
# runtime. When called with the --full-native flag, it additionally
# bundles pre-built Rust CLI binaries so that the resulting npm package can run
# the native implementation when users set CODEX_RUST=1.
#
# Usage
#   install_native_deps.sh [RELEASE_ROOT] [--full-native]
#
# The optional RELEASE_ROOT is the path that contains package.json.  Omitting
# it installs the binaries into the repository's own bin/ folder to support
# local development.

set -euo pipefail

# ------------------
# Parse arguments
# ------------------

DEST_DIR=""
INCLUDE_RUST=0

for arg in "$@"; do
  case "$arg" in
    --full-native)
      INCLUDE_RUST=1
      ;;
    *)
      if [[ -z "$DEST_DIR" ]]; then
        DEST_DIR="$arg"
      else
        echo "Unexpected argument: $arg" >&2
        exit 1
      fi
      ;;
  esac
done

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
WORKFLOW_URL="https://github.com/openai/codex/actions/runs/14950726936"
WORKFLOW_ID="${WORKFLOW_URL##*/}"

ARTIFACTS_DIR="$(mktemp -d)"
trap 'rm -rf "$ARTIFACTS_DIR"' EXIT

# NB: The GitHub CLI `gh` must be installed and authenticated.
gh run download --dir "$ARTIFACTS_DIR" --repo openai/codex "$WORKFLOW_ID"

# Decompress the artifacts for Linux sandboxing.
zstd -d "$ARTIFACTS_DIR/x86_64-unknown-linux-musl/codex-linux-sandbox-x86_64-unknown-linux-musl.zst" \
     -o "$BIN_DIR/codex-linux-sandbox-x64"

zstd -d "$ARTIFACTS_DIR/aarch64-unknown-linux-gnu/codex-linux-sandbox-aarch64-unknown-linux-gnu.zst" \
     -o "$BIN_DIR/codex-linux-sandbox-arm64"

if [[ "$INCLUDE_RUST" -eq 1 ]]; then
  # x64 Linux
  zstd -d "$ARTIFACTS_DIR/x86_64-unknown-linux-musl/codex-x86_64-unknown-linux-musl.zst" \
      -o "$BIN_DIR/codex-x86_64-unknown-linux-musl"
  # ARM64 Linux
  zstd -d "$ARTIFACTS_DIR/aarch64-unknown-linux-gnu/codex-aarch64-unknown-linux-gnu.zst" \
      -o "$BIN_DIR/codex-aarch64-unknown-linux-gnu"
  # x64 macOS
  zstd -d "$ARTIFACTS_DIR/x86_64-apple-darwin/codex-x86_64-apple-darwin.zst" \
      -o "$BIN_DIR/codex-x86_64-apple-darwin"
  # ARM64 macOS
  zstd -d "$ARTIFACTS_DIR/aarch64-apple-darwin/codex-aarch64-apple-darwin.zst" \
      -o "$BIN_DIR/codex-aarch64-apple-darwin"
fi

echo "Installed native dependencies into $BIN_DIR"
