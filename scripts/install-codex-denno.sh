#!/usr/bin/env bash
set -euo pipefail

# Install a locally built Codex CLI binary as `codex-denno`.
# Usage:
#   scripts/install-codex-denno.sh [INSTALL_DIR]
# Default INSTALL_DIR is "$HOME/.local/bin".

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

INSTALL_DIR="${1:-"$HOME/.local/bin"}"
BIN_NAME="codex-denno"

echo "==> Building Codex CLI (codex-rs/codex-cli) ..."
(
  cd "${REPO_ROOT}/codex-rs"
  cargo build -p codex-cli --release
)

SRC_BIN="${REPO_ROOT}/codex-rs/target/release/codex"
if [ ! -x "${SRC_BIN}" ]; then
  echo "ERROR: built codex binary not found at ${SRC_BIN}" >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}"
cp "${SRC_BIN}" "${INSTALL_DIR}/${BIN_NAME}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}"

echo "==> Installed ${BIN_NAME} to ${INSTALL_DIR}"
echo ""
echo "To use it from your shell, ensure ${INSTALL_DIR} is on your PATH, e.g.:"
echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""

