#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
codex_rs_dir="$(cd -- "${script_dir}/.." && pwd)"
cd "${codex_rs_dir}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cargo run -p codex-app-server-protocol-stable-export -- --out "$tmp"

ts_dir="app-server-ts-types/stable"
json_dir="app-server-json-schema/stable"

rm -rf "$ts_dir" "$json_dir"
mkdir -p "$ts_dir" "$json_dir"

rsync -a --include "*/" --include "*.ts" --exclude "*" "$tmp"/ "$ts_dir"/
rsync -a --include "*/" --include "*.json" --exclude "*" "$tmp"/ "$json_dir"/

