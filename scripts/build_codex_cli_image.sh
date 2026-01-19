#!/usr/bin/env bash

set -euo pipefail

# Usage: ./build_codex_cli_image.sh <version>
# If no version is provided, default to "latest"
VERSION="${1:-latest}"

echo "Building Codex CLI Docker image with version: $VERSION"

cd "codex-rs"
cargo build --target x86_64-unknown-linux-gnu --release --bin codex

docker build -t "capcr.azurecr.io/codex/codex-cli:$VERSION" -f- . <<'EOF'
FROM scratch
COPY target/x86_64-unknown-linux-gnu/release/codex /codex
ENTRYPOINT ["/codex"]
EOF

docker push capcr.azurecr.io/codex/codex-cli:"$VERSION"