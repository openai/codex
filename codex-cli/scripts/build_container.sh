#!/bin/bash

set -euo pipefail

SCRIPT_DIR=$(realpath "$(dirname "$0")")
pushd "$SCRIPT_DIR/.." >> /dev/null || {
  echo "Error: Failed to change directory to $SCRIPT_DIR/.."
  exit 1
}
trap 'popd >> /dev/null' EXIT

./scripts/stage_container_package.sh
docker build -t codex -f "./Dockerfile" .
