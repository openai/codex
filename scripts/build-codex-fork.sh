#!/usr/bin/env bash
# Build the forked Codex CLI and install to tools/bin.
set -euo pipefail
root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root_dir/codex-rs"
echo "[build] cargo build --release -p codex-cli"
cargo build --release -p codex-cli

mkdir -p "$root_dir/tools/bin"
src="$(pwd)/target/release/codex"
dst="$root_dir/tools/bin/codex"
cp -f "$src" "$dst"
chmod +x "$dst"
echo "Installed forked codex → $dst"
echo "Add to PATH for this repo: export PATH=\"$root_dir/tools/bin:\$PATH\""

