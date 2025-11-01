#!/usr/bin/env bash
set -euo pipefail

# Native Linux GNU build for Codex CLI on x86_64-unknown-linux-gnu.
#
# This script is intended to be run on an x86_64 Linux host (or in an
# x86_64 container/VM). It builds the `codex` binary and packages it into a
# tarball matching the GitHub Releases naming:
#   codex-x86_64-unknown-linux-gnu.tar.gz
#
# Usage:
#   cd codex-rs
#   ./scripts/build-linux-gnu.sh
#
# Optional: Pin the toolchain via the workspace rust-toolchain.toml (default).

if [[ "${OSTYPE}" != linux* ]]; then
  echo "[!] This script is for native Linux builds. Detected OSTYPE='${OSTYPE}'." >&2
  echo "    Run this inside a Linux VM/container or use the Docker approach described in the README." >&2
  exit 2
fi

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT_DIR"

BIN=codex
TARGET=x86_64-unknown-linux-gnu
OUTDIR="${ROOT_DIR}/dist/linux-gnu/${TARGET}"
mkdir -p "$OUTDIR"

TOOLCHAIN=${RUSTUP_TOOLCHAIN:-"$(grep -Eo 'channel\s*=\s*"[^"]+"' rust-toolchain.toml 2>/dev/null | sed -E 's/.*"(.*)"/\1/; s/channel\s*=\s*//g' || true)"}
if [[ -n "$TOOLCHAIN" ]]; then
  export RUSTUP_TOOLCHAIN="$TOOLCHAIN"
  echo "==> Using toolchain: $RUSTUP_TOOLCHAIN"
else
  echo "==> Using default rustup toolchain"
fi

echo "==> Ensuring target installed: $TARGET"
rustup target add "$TARGET" >/dev/null

echo "==> Building $BIN for $TARGET (release)"
cargo build --release --bin "$BIN" --target "$TARGET"

SRC_BIN="${ROOT_DIR}/target/${TARGET}/release/${BIN}"
if [[ ! -f "$SRC_BIN" ]]; then
  echo "Build succeeded but binary not found: $SRC_BIN" >&2
  exit 1
fi

FINAL_NAME="${BIN}-${TARGET}"
cp -f "$SRC_BIN" "$OUTDIR/$FINAL_NAME"
tar -C "$OUTDIR" -czf "$OUTDIR/${FINAL_NAME}.tar.gz" "$FINAL_NAME"

echo "==> Artifact: $OUTDIR/${FINAL_NAME}.tar.gz"

