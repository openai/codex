#!/usr/bin/env bash
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_dir="$(cd "$crate_dir/.." && pwd)"

cd "$workspace_dir"

cargo build \
  -p codex-wasm-harness \
  --target wasm32-unknown-unknown

wasm-bindgen \
  --target web \
  --out-dir "$crate_dir/examples/pkg" \
  "$workspace_dir/target/wasm32-unknown-unknown/debug/codex_wasm_harness.wasm"
