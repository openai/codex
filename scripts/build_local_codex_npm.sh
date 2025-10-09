#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
CODEX_RS_DIR="$REPO_ROOT/codex-rs"

RELEASE_VERSION="${1:-0.0.0-local}"
PACK_OUTPUT="$REPO_ROOT/dist/codex-super-${RELEASE_VERSION}.tgz"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Error: Required command '$1' not found in PATH." >&2
    exit 1
  fi
}

require_cmd uv
require_cmd cargo
require_cmd rustc

if ! rustup target list --installed | grep -q '^x86_64-unknown-linux-musl$'; then
  echo "Error: Target 'x86_64-unknown-linux-musl' is not installed. Run 'rustup target add x86_64-unknown-linux-musl' first." >&2
  exit 1
fi

if [[ ! -d "$CODEX_RS_DIR" ]]; then
  echo "Error: Expected directory not found: $CODEX_RS_DIR" >&2
  exit 1
fi

MUSL_LINKER="${MUSL_LINKER:-x86_64-linux-musl-gcc}"
MUSL_CXX="${MUSL_CXX:-x86_64-linux-musl-g++}"
MUSL_AR="${MUSL_AR:-x86_64-linux-musl-ar}"

if ! command -v "$MUSL_LINKER" >/dev/null 2>&1; then
  cat >&2 <<EOF
Error: '$MUSL_LINKER' not found in PATH. Install the musl cross toolchain, e.g.:
  brew install FiloSottile/musl-cross/musl-cross
or set MUSL_LINKER to the desired linker binary.
EOF
  exit 1
fi

if ! command -v "$MUSL_CXX" >/dev/null 2>&1; then
  cat >&2 <<EOF
Error: '$MUSL_CXX' not found in PATH. Install the musl cross toolchain or set MUSL_CXX.
EOF
  exit 1
fi

if ! command -v "$MUSL_AR" >/dev/null 2>&1; then
  cat >&2 <<EOF
Error: '$MUSL_AR' not found in PATH. Install the musl cross toolchain or set MUSL_AR.
EOF
  exit 1
fi

cd "$REPO_ROOT"

echo "==> Installing latest native dependencies into vendor/"
uv run codex-cli/scripts/install_native_deps.py codex-cli

echo "==> Building codex-cli (host release)"
(
  cd "$CODEX_RS_DIR"
  cargo build --release -p codex-cli
)

echo "==> Building codex-cli (x86_64-unknown-linux-musl release)"
(
  cd "$CODEX_RS_DIR"
  CC_x86_64_unknown_linux_musl="$MUSL_LINKER" \
  CXX_x86_64_unknown_linux_musl="$MUSL_CXX" \
  AR_x86_64_unknown_linux_musl="$MUSL_AR" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$MUSL_LINKER" \
  cargo build --release -p codex-cli --target x86_64-unknown-linux-musl
)

update_vendor_binary() {
  local src="$1"
  local dest="$2"

  if [[ ! -f "$src" ]]; then
    echo "Error: Built binary not found: $src" >&2
    exit 1
  fi

  local dest_dir
  dest_dir="$(dirname "$dest")"

  if [[ ! -d "$dest_dir" ]]; then
    echo "Error: Expected vendor directory missing: $dest_dir" >&2
    exit 1
  fi

  cp "$src" "$dest"
  chmod +x "$dest"
  echo "  updated $dest"
}

HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
HOST_SRC="$REPO_ROOT/codex-rs/target/release/codex"
HOST_DEST="$REPO_ROOT/codex-cli/vendor/$HOST_TRIPLE/codex/codex"

if [[ -d "$(dirname "$HOST_DEST")" ]]; then
  echo "==> Updating vendor binary for host target ($HOST_TRIPLE)"
  update_vendor_binary "$HOST_SRC" "$HOST_DEST"
else
  echo "==> Skipping host vendor update (directory missing for $HOST_TRIPLE)"
fi

MUSL_SRC="$REPO_ROOT/codex-rs/target/x86_64-unknown-linux-musl/release/codex"
MUSL_DEST="$REPO_ROOT/codex-cli/vendor/x86_64-unknown-linux-musl/codex/codex"

echo "==> Updating vendor binary for x86_64-unknown-linux-musl"
update_vendor_binary "$MUSL_SRC" "$MUSL_DEST"

mkdir -p "$(dirname "$PACK_OUTPUT")"

echo "==> Building npm package (version $RELEASE_VERSION)"
uv run codex-cli/scripts/build_npm_package.py \
  --package codex-super \
  --release-version "$RELEASE_VERSION" \
  --vendor-src "$REPO_ROOT/codex-cli/vendor" \
  --pack-output "$PACK_OUTPUT"

echo "==> npm package ready: $PACK_OUTPUT"
