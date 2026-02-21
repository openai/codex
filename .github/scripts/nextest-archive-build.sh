#!/usr/bin/env bash
set -euo pipefail

archive_file="$1"
target="${2:-x86_64-unknown-linux-gnu}"
cargo_profile="${3:-ci-test}"

cargo nextest archive \
  --all-features \
  --target "$target" \
  --cargo-profile "$cargo_profile" \
  --timings \
  --archive-file "$archive_file"
