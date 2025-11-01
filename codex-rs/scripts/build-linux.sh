#!/usr/bin/env bash
set -euo pipefail

# Cross-compile selected Codex binaries for Linux (musl) from macOS.
#
# Usage examples:
#   scripts/build-linux.sh app-server
#   scripts/build-linux.sh codex app-server
#   scripts/build-linux.sh all
#
# Produces artifacts under: codex-rs/dist/linux/<target>/<binary>

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT_DIR"

BINARIES=("codex-app-server")
if [[ $# -gt 0 ]]; then
  if [[ "$1" == "all" ]]; then
    BINARIES=("codex" "codex-app-server")
  else
    BINARIES=()
    for arg in "$@"; do
      case "$arg" in
        codex) BINARIES+=("codex");;
        app-server) BINARIES+=("codex-app-server");;
        *) echo "Unknown binary: $arg" >&2; exit 2;;
      esac
    done
  fi
fi

echo "==> Using rustup toolchain from rust-toolchain.toml (if present)"
TOOLCHAIN=${RUSTUP_TOOLCHAIN:-"$(grep -Eo 'channel\s*=\s*"[^"]+"' rust-toolchain.toml 2>/dev/null | sed -E 's/.*"(.*)"/\1/; s/channel\s*=\s*//g' || true)"}
if [[ -n "$TOOLCHAIN" ]]; then
  export RUSTUP_TOOLCHAIN="$TOOLCHAIN"
  echo "==> TOOLCHAIN=$RUSTUP_TOOLCHAIN"
fi

echo "==> Installing musl targets"
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl >/dev/null

TARGETS=("x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl")

mkdir -p "$ROOT_DIR/dist/linux"

for T in "${TARGETS[@]}"; do
  for BIN in "${BINARIES[@]}"; do
    echo "==> Building $BIN for $T (release)"
    RUSTFLAGS="-C target-feature=-crt-static" \
      cargo build --release --bin "$BIN" --target "$T"

    OUTDIR="$ROOT_DIR/dist/linux/$T"
    mkdir -p "$OUTDIR"
    cp -f "$ROOT_DIR/target/$T/release/$BIN" "$OUTDIR/" || true
    echo "     -> $OUTDIR/$BIN"
  done
done

echo "==> Done"

