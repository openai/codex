#!/usr/bin/env bash
# CI script to verify rustls-only (no native-tls) is being used
# This is critical for mTLS support to work correctly

set -euo pipefail

echo "=== Checking TLS backend configuration ==="
echo ""

output=$(cargo tree -p codex-tui -i openssl-sys 2>&1 || true)

if echo "$output" | grep -q "warning: nothing to print"; then
    echo "✓ PASS: Using rustls-only (no native-tls/openssl-sys found)"
    echo ""
    echo "mTLS support will work correctly."
    exit 0
else
    echo "✗ FAIL: native-tls detected in dependency tree!"
    echo ""
    echo "codex requires rustls-only for mTLS support to work correctly."
    echo "The code uses Identity::from_pem() which only exists with rustls."
    echo ""
    echo "Dependency tree showing openssl-sys:"
    echo "---"
    echo "$output"
    echo "---"
    echo ""
    echo "To fix this issue:"
    echo "  1. Check workspace Cargo.toml has:"
    echo "     reqwest = { version = \"0.12\", default-features = false, features = [\"rustls-tls\"] }"
    echo ""
    echo "  2. Check that NO other crate enables native-tls or default-tls features"
    echo "     Run: rg 'reqwest.*default-tls|reqwest.*native-tls' --type toml"
    echo ""
    echo "  3. Check sentry crate uses rustls:"
    echo "     sentry = { version = \"0.34\", default-features = false, features = [\"rustls\", ...] }"
    echo ""
    echo "  4. Verify all individual crate Cargo.toml files that override reqwest"
    echo "     have: default-features = false, features = [\"rustls-tls\", ...]"
    echo ""
    exit 1
fi
