#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
TARGET="aarch64-linux-android"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing command: $1" >&2
    exit 1
  fi
}

need_cmd cargo
need_cmd rustc

HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')
if [ "$HOST_TARGET" != "$TARGET" ]; then
  if command -v rustup >/dev/null 2>&1; then
    if ! rustup target list --installed | grep -qx "$TARGET"; then
      cat >&2 <<EOF
Rust target '$TARGET' is not installed.
Install it with:
  rustup target add $TARGET
EOF
      exit 1
    fi
  else
    cat >&2 <<EOF
Host target is '$HOST_TARGET', not '$TARGET', and rustup is unavailable.
Install rustup and add the target:
  rustup target add $TARGET
EOF
    exit 1
  fi
fi

cd "$ROOT_DIR/codex-rs"
echo "[termux-check] cargo check -p codex-cli --target $TARGET"
cargo check -p codex-cli --target "$TARGET"
echo "[termux-check] cargo check -p codex-tui --target $TARGET"
cargo check -p codex-tui --target "$TARGET"
echo "[termux-check] done"
