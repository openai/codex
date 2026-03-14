#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
TARGET="aarch64-linux-android"
PROFILE="release"

if [ "${1:-}" = "--debug" ] || [ "${1:-}" = "debug" ]; then
  PROFILE="debug"
fi

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing command: $1" >&2
    exit 1
  fi
}

need_cmd cargo
need_cmd rustc

cores=$(nproc 2>/dev/null || echo 1)
mem_kb=$(awk '/MemAvailable:/ { print $2 }' /proc/meminfo 2>/dev/null || echo 0)

# Conservative job tuning for mobile memory pressure.
if [ "${mem_kb:-0}" -lt 3000000 ]; then
  jobs=1
elif [ "${mem_kb:-0}" -lt 5000000 ]; then
  jobs=2
else
  jobs=$((cores / 2))
  [ "$jobs" -lt 2 ] && jobs=2
  [ "$jobs" -gt 4 ] && jobs=4
fi
# Enforce serialized rustc/linker work to avoid parallel compiler pressure.
jobs=1

data_use_pct=$(df -P /data 2>/dev/null | awk 'NR==2 { gsub(/%/, "", $5); print $5 }')
if [ -n "${data_use_pct:-}" ] && [ "${data_use_pct:-0}" -ge 92 ]; then
  echo "[termux-build-safe] warning: /data usage is ${data_use_pct}% (low free space may cause instability)." >&2
fi

echo "[termux-build-safe] target: $TARGET"
echo "[termux-build-safe] cores: $cores"
echo "[termux-build-safe] MemAvailable: ${mem_kb} kB"
echo "[termux-build-safe] CARGO_BUILD_JOBS=$jobs (forced serial rustc/link; no parallel rustc)"
echo "[termux-build-safe] CARGO_BUILD_PIPELINING=false (avoid rustc/link overlap)"
if [ "$PROFILE" = "release" ]; then
  echo "[termux-build-safe] release overrides: opt-level=2, LTO=off, codegen-units=2, debug=0"
fi
RUSTFLAGS_BASE="-C llvm-args=--threads=1"
if [ "$PROFILE" = "release" ]; then
  RUSTFLAGS="$RUSTFLAGS_BASE"
else
  RUSTFLAGS="$RUSTFLAGS_BASE -C codegen-units=1 -C debuginfo=0 -C opt-level=0 -C panic=abort -C embed-bitcode=no"
fi

echo "[termux-build-safe] profile: $PROFILE"
echo "[termux-build-safe] rustflags: $RUSTFLAGS"

cd "$ROOT_DIR/codex-rs"

run_cargo() {
  CARGO_BUILD_JOBS="$jobs" \
  CARGO_BUILD_PIPELINING=false \
  RUSTFLAGS="$RUSTFLAGS" \
  "$@"
}

if [ "$PROFILE" = "release" ]; then
  CARGO_PROFILE_RELEASE_OPT_LEVEL=2 \
  CARGO_PROFILE_RELEASE_LTO=off \
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=2 \
  CARGO_PROFILE_RELEASE_DEBUG=0 \
  run_cargo cargo build --release -p codex-cli -p codex-exec --target "$TARGET"
else
  CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_PROFILE_DEV_INCREMENTAL=false \
  CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=false \
  CARGO_PROFILE_DEV_OVERFLOW_CHECKS=false \
  CARGO_PROFILE_DEV_PANIC=abort \
  CARGO_PROFILE_DEV_CODEGEN_UNITS=1 \
  run_cargo cargo build -p codex-cli -p codex-exec --target "$TARGET"
fi

echo "[termux-build-safe] done"
echo "[termux-build-safe] binaries:"
echo "  $ROOT_DIR/codex-rs/target/$TARGET/$PROFILE/codex"
echo "  $ROOT_DIR/codex-rs/target/$TARGET/$PROFILE/codex-exec"
