#!/usr/bin/env bash

# Install native runtime dependencies for codex-cli.
#
# Usage
#   install_native_deps.sh [--workflow-url URL] [CODEX_CLI_ROOT]
#
# The optional RELEASE_ROOT is the path that contains package.json.  Omitting
# it installs the binaries into the repository's own bin/ folder to support
# local development.

set -euo pipefail

# ------------------
# Parse arguments
# ------------------

CODEX_CLI_ROOT=""

# Until we start publishing stable GitHub releases, we have to grab the binaries
# from the GitHub Action that created them. The calling workflow passes the
# rust-release run URL via --workflow-url. Fallback shown below is only a
# placeholder and not used in CI.
WORKFLOW_URL="https://github.com/openai/codex/actions/runs/17417194663" # placeholder
REPO_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workflow-url)
      shift || { echo "--workflow-url requires an argument"; exit 1; }
      if [ -n "$1" ]; then
        WORKFLOW_URL="$1"
      fi
      ;;
    --repo)
      shift || { echo "--repo requires an argument"; exit 1; }
      if [ -n "$1" ]; then
        REPO_OVERRIDE="$1"
      fi
      ;;
    *)
      if [[ -z "$CODEX_CLI_ROOT" ]]; then
        CODEX_CLI_ROOT="$1"
      else
        echo "Unexpected argument: $1" >&2
        exit 1
      fi
      ;;
  esac
  shift
done

# ----------------------------------------------------------------------------
# Determine where the binaries should be installed.
# ----------------------------------------------------------------------------

if [ -n "$CODEX_CLI_ROOT" ]; then
  # The caller supplied a release root directory.
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

WORKFLOW_ID="${WORKFLOW_URL##*/}"

# Derive owner/repo from the workflow URL if not explicitly provided.
# Expected formats:
#   https://github.com/<owner>/<repo>/actions/runs/<id>
REPO="$REPO_OVERRIDE"
if [ -z "$REPO" ]; then
  # Strip protocol and domain
  # shellcheck disable=SC2001
  PATH_PARTS="$(echo "$WORKFLOW_URL" | sed -E 's#https?://github.com/##')"
  # Extract first two path segments as owner/repo
  OWNER="$(echo "$PATH_PARTS" | cut -d'/' -f1)"
  NAME="$(echo "$PATH_PARTS" | cut -d'/' -f2)"
  if [ -n "$OWNER" ] && [ -n "$NAME" ]; then
    REPO="$OWNER/$NAME"
  else
    REPO="openai/codex"
  fi
fi

ARTIFACTS_DIR="$(mktemp -d)"
trap 'rm -rf "$ARTIFACTS_DIR"' EXIT

# NB: The GitHub CLI `gh` must be installed and authenticated.
gh run download --dir "$ARTIFACTS_DIR" --repo "$REPO" "$WORKFLOW_ID"

# x64 Linux
zstd -d "$ARTIFACTS_DIR/x86_64-unknown-linux-musl/codex-x86_64-unknown-linux-musl.zst" \
    -o "$BIN_DIR/codex-x86_64-unknown-linux-musl"
# ARM64 Linux
zstd -d "$ARTIFACTS_DIR/aarch64-unknown-linux-musl/codex-aarch64-unknown-linux-musl.zst" \
    -o "$BIN_DIR/codex-aarch64-unknown-linux-musl"
# x64 macOS
zstd -d "$ARTIFACTS_DIR/x86_64-apple-darwin/codex-x86_64-apple-darwin.zst" \
    -o "$BIN_DIR/codex-x86_64-apple-darwin"
# ARM64 macOS
zstd -d "$ARTIFACTS_DIR/aarch64-apple-darwin/codex-aarch64-apple-darwin.zst" \
    -o "$BIN_DIR/codex-aarch64-apple-darwin"
# x64 Windows
zstd -d "$ARTIFACTS_DIR/x86_64-pc-windows-msvc/codex-x86_64-pc-windows-msvc.exe.zst" \
    -o "$BIN_DIR/codex-x86_64-pc-windows-msvc.exe"
# ARM64 Windows
zstd -d "$ARTIFACTS_DIR/aarch64-pc-windows-msvc/codex-aarch64-pc-windows-msvc.exe.zst" \
    -o "$BIN_DIR/codex-aarch64-pc-windows-msvc.exe"

echo "Installed native dependencies into $BIN_DIR"
