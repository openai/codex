#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../codex-rs"

bench_targets="$(
  cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] as $package | $package.targets[] | select(any(.kind[]; . == "bench")) | [$package.name, .name] | @tsv'
)"

if [[ -n "$bench_targets" ]]; then
  while IFS=$'\t' read -r package bench_target; do
    cargo bench -p "$package" --bench "$bench_target" -- --test
  done <<< "$bench_targets"
fi
