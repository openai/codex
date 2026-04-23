#!/bin/bash

set -euo pipefail

SCRIPT_DIR=$(realpath "$(dirname "$0")")
pushd "$SCRIPT_DIR/.." >> /dev/null || {
  echo "Error: Failed to change directory to $SCRIPT_DIR/.." >&2
  exit 1
}

BUILD_ROOT=$(mktemp -d -t codex-container-stage-XXXXXX)
trap 'rm -rf "$BUILD_ROOT"; popd >> /dev/null' EXIT

CODEX_VERSION=$(node -p "require('./package.json').version")

docker_target_triple() {
  case "$(uname -m)" in
    x86_64)
      echo "x86_64-unknown-linux-musl"
      ;;
    arm64|aarch64)
      echo "aarch64-unknown-linux-musl"
      ;;
    *)
      echo "Error: Unsupported architecture for Docker packaging: $(uname -m)" >&2
      exit 1
      ;;
  esac
}

resolve_release_run_id() {
  gh run list \
    --repo openai/codex \
    --workflow .github/workflows/rust-release.yml \
    --limit 10 \
    --json databaseId,status,conclusion \
    --jq 'map(select(.status == "completed" and .conclusion == "success")) | .[0].databaseId'
}

find_local_linux_binary() {
  local target_triple="$1"

  if [[ "$(uname -s)" != "Linux" ]]; then
    return 1
  fi

  local candidate="../codex-rs/target/$target_triple/release/codex"
  [[ -x "$candidate" ]] || return 1

  echo "$candidate"
}

copy_binary_into_vendor() {
  local binary_path="$1"
  local target_triple="$2"
  local vendor_dir="$BUILD_ROOT/vendor/$target_triple/codex"

  mkdir -p "$vendor_dir"
  cp "$binary_path" "$vendor_dir/codex"
  chmod +x "$vendor_dir/codex"
}

download_release_binary() {
  local run_id="$1"
  local target_triple="$2"
  local artifact_dir="$BUILD_ROOT/artifacts/$target_triple"
  local archive
  local extracted_binary

  mkdir -p "$artifact_dir"
  gh run download \
    --repo openai/codex \
    "$run_id" \
    -n "$target_triple" \
    --dir "$artifact_dir" \
    > /dev/null

  archive=$(find "$artifact_dir" -maxdepth 1 -name "codex-${target_triple}.tar.gz" | head -1)
  if [[ -z "$archive" ]]; then
    echo "Error: Failed to find a Codex archive for $target_triple in run $run_id." >&2
    exit 1
  fi

  tar -xzf "$archive" -C "$artifact_dir"
  extracted_binary="$artifact_dir/codex-${target_triple}"
  if [[ ! -x "$extracted_binary" ]]; then
    echo "Error: Extracted Codex binary is missing or not executable: $extracted_binary" >&2
    exit 1
  fi

  copy_binary_into_vendor "$extracted_binary" "$target_triple"
}

TARGET_TRIPLE=$(docker_target_triple)

if LOCAL_BINARY=$(find_local_linux_binary "$TARGET_TRIPLE"); then
  copy_binary_into_vendor "$LOCAL_BINARY" "$TARGET_TRIPLE"
else
  RUN_ID=$(resolve_release_run_id)
  if [[ -z "$RUN_ID" || "$RUN_ID" == "null" ]]; then
    echo "Error: Failed to resolve a successful rust-release run." >&2
    exit 1
  fi

  download_release_binary "$RUN_ID" "$TARGET_TRIPLE"
fi

rm -f ./dist/codex.tgz ./dist/openai-codex-*.tgz

python3 ./scripts/build_npm_package.py \
  --package codex \
  --version "$CODEX_VERSION" \
  --staging-dir "$BUILD_ROOT/staging/codex" \
  --pack-output ./dist/codex.tgz \
  --vendor-src "$BUILD_ROOT/vendor"

corepack pnpm install --dir ./container-install --lockfile-only
