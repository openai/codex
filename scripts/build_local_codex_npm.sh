#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
CODEX_RS_DIR="$REPO_ROOT/codex-rs"

RELEASE_VERSION="0.0.0-local"
TARGETS_RAW="host,x86_64-unknown-linux-musl"

print_usage() {
  cat <<'EOF'
Usage: build_local_codex_npm.sh [--targets <list>] [release-version]

  --targets <list>  Comma-separated targets to build. Supported values:
                    host, x86_64-unknown-linux-musl
  release-version   Optional version string for the generated npm package.
                    Defaults to 0.0.0-local
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --targets)
      shift
      if [[ $# -eq 0 ]]; then
        echo "Error: --targets requires an argument" >&2
        print_usage >&2
        exit 1
      fi
      TARGETS_RAW="$1"
      ;;
    --targets=*)
      TARGETS_RAW="${1#*=}"
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    --*)
      echo "Error: Unknown option: $1" >&2
      print_usage >&2
      exit 1
      ;;
    *)
      if [[ "$RELEASE_VERSION" != "0.0.0-local" ]]; then
        echo "Error: Multiple release versions provided" >&2
        print_usage >&2
        exit 1
      fi
      RELEASE_VERSION="$1"
      ;;
  esac
  shift
done

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

if [[ ! -d "$CODEX_RS_DIR" ]]; then
  echo "Error: Expected directory not found: $CODEX_RS_DIR" >&2
  exit 1
fi

IFS=',' read -r -a TARGETS <<<"$TARGETS_RAW"

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  echo "Error: --targets must not be empty" >&2
  exit 1
fi

NEED_HOST=false
NEED_X86_MUSL=false
AUTO_ADDED_MUSL=false
COPY_HOST_TO_MUSL=false

for target in "${TARGETS[@]}"; do
  case "$target" in
    host)
      NEED_HOST=true
      ;;
    x86_64-unknown-linux-musl)
      NEED_X86_MUSL=true
      ;;
    *)
      echo "Error: Unsupported target: $target" >&2
      exit 1
      ;;
  esac
done

HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"

if "$NEED_HOST" && [[ "$HOST_TRIPLE" == *-unknown-linux-gnu ]]; then
  if ! "$NEED_X86_MUSL"; then
    echo "==> Detected Linux host ($HOST_TRIPLE); building x86_64-unknown-linux-musl as well so packaged CLI includes the binary it uses at runtime."
    NEED_X86_MUSL=true
    AUTO_ADDED_MUSL=true
  fi
fi

if "$NEED_X86_MUSL"; then
  if ! rustup target list --installed | grep -q '^x86_64-unknown-linux-musl$'; then
    echo "Error: Target 'x86_64-unknown-linux-musl' is not installed. Run 'rustup target add x86_64-unknown-linux-musl' first." >&2
    exit 1
  fi

  MUSL_LINKER="${MUSL_LINKER:-x86_64-linux-musl-gcc}"
  MUSL_CXX="${MUSL_CXX:-x86_64-linux-musl-g++}"
  MUSL_AR="${MUSL_AR:-x86_64-linux-musl-ar}"

  MISSING_TOOL=""
  if ! command -v "$MUSL_LINKER" >/dev/null 2>&1; then
    MISSING_TOOL="$MUSL_LINKER"
  elif ! command -v "$MUSL_CXX" >/dev/null 2>&1; then
    MISSING_TOOL="$MUSL_CXX"
  elif ! command -v "$MUSL_AR" >/dev/null 2>&1; then
    MISSING_TOOL="$MUSL_AR"
  fi

  if [[ -n "$MISSING_TOOL" ]]; then
    if "$AUTO_ADDED_MUSL"; then
      echo "==> Warning: musl toolchain component '$MISSING_TOOL' not found. Skipping musl build and copying host binary instead."
      COPY_HOST_TO_MUSL=true
      NEED_X86_MUSL=false
    else
      cat >&2 <<EOF
Error: '$MISSING_TOOL' not found in PATH. Install the musl cross toolchain or set MUSL_LINKER/MUSL_CXX/MUSL_AR.
EOF
      exit 1
    fi
  fi
fi

cd "$REPO_ROOT"

echo "==> Installing latest native dependencies into vendor/"
uv run codex-cli/scripts/install_native_deps.py codex-cli

if "$NEED_HOST"; then
  echo "==> Building codex-cli (host release)"
  (
    cd "$CODEX_RS_DIR"
    cargo build --release -p codex-cli
  )
fi

if "$NEED_X86_MUSL"; then
  echo "==> Building codex-cli (x86_64-unknown-linux-musl release)"
  (
    cd "$CODEX_RS_DIR"
    CC_x86_64_unknown_linux_musl="$MUSL_LINKER" \
    CXX_x86_64_unknown_linux_musl="$MUSL_CXX" \
    AR_x86_64_unknown_linux_musl="$MUSL_AR" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$MUSL_LINKER" \
    cargo build --release -p codex-cli --target x86_64-unknown-linux-musl
  )
fi

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

HOST_SRC="$REPO_ROOT/codex-rs/target/release/codex"

if "$NEED_HOST"; then
  HOST_DEST="$REPO_ROOT/codex-cli/vendor/$HOST_TRIPLE/codex/codex"

  if [[ -d "$(dirname "$HOST_DEST")" ]]; then
    echo "==> Updating vendor binary for host target ($HOST_TRIPLE)"
    update_vendor_binary "$HOST_SRC" "$HOST_DEST"
  else
    echo "==> Skipping host vendor update (directory missing for $HOST_TRIPLE)"
  fi
fi

if "$COPY_HOST_TO_MUSL"; then
  MUSL_DEST="$REPO_ROOT/codex-cli/vendor/x86_64-unknown-linux-musl/codex/codex"
  echo "==> Copying host binary to $MUSL_DEST (musl toolchain unavailable)"
  mkdir -p "$(dirname "$MUSL_DEST")"
  update_vendor_binary "$HOST_SRC" "$MUSL_DEST"
fi

if "$NEED_X86_MUSL"; then
  MUSL_SRC="$REPO_ROOT/codex-rs/target/x86_64-unknown-linux-musl/release/codex"
  MUSL_DEST="$REPO_ROOT/codex-cli/vendor/x86_64-unknown-linux-musl/codex/codex"

  echo "==> Updating vendor binary for x86_64-unknown-linux-musl"
  update_vendor_binary "$MUSL_SRC" "$MUSL_DEST"
fi

mkdir -p "$(dirname "$PACK_OUTPUT")"

echo "==> Building npm package (version $RELEASE_VERSION)"
uv run codex-cli/scripts/build_npm_package.py \
  --package codex-super \
  --release-version "$RELEASE_VERSION" \
  --vendor-src "$REPO_ROOT/codex-cli/vendor" \
  --pack-output "$PACK_OUTPUT"

echo "==> npm package ready: $PACK_OUTPUT"
