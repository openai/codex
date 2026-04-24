#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
proto_dir="$repo_root/codex-rs/config/src/thread_config/proto"
generated="$proto_dir/codex.thread_config.v1.rs"
tmpdir="$(mktemp -d)"

cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup EXIT

(
    cd "$repo_root/codex-rs"
    CARGO_TARGET_DIR="$tmpdir/target" cargo run \
        -p codex-config \
        --example generate-proto \
        -- "$proto_dir"
)

if ! sed -n '2p' "$generated" | grep -q 'clippy::trivially_copy_pass_by_ref'; then
    {
        sed -n '1p' "$generated"
        printf '#![allow(clippy::trivially_copy_pass_by_ref)]\n'
        sed '1d' "$generated"
    } > "$tmpdir/generated.rs"
    mv "$tmpdir/generated.rs" "$generated"
fi

rustfmt --edition 2024 "$generated"

awk '
    NR == 3 && previous ~ /clippy::trivially_copy_pass_by_ref/ && $0 != "" { print "" }
    { print; previous = $0 }
' "$generated" > "$tmpdir/formatted.rs"
mv "$tmpdir/formatted.rs" "$generated"
