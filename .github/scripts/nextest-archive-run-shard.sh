#!/usr/bin/env bash
set -euo pipefail

archive_file="$1"
workspace_remap="$2"
partition_spec="$3"

cargo nextest run \
  --archive-file "$archive_file" \
  --workspace-remap "$workspace_remap" \
  --partition "$partition_spec" \
  --no-fail-fast
