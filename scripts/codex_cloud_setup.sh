#!/bin/bash
# Cloud Codex Setup Script
# Configures cloud-based Codex integration

set -euo pipefail

echo "☁️  Starting cloud Codex setup..."

# Check if we're in the right directory
if [[ ! -f "package.json" ]] || [[ ! -d "codex-rs" ]]; then
    echo "Error: Please run this script from the root of the codex repository"
    exit 1
fi

# Load environment variables if .env exists
if [[ -f ".env" ]]; then
    echo "📄 Loading environment from .env"
    set -a
    source .env
    set +a
fi

# Set default values for cloud Codex configuration
export CODEX_MODE="${CODEX_MODE:-cloud}"
export CODEX_RESONANCE="${CODEX_RESONANCE:-full}"
export CODEX_CLOUD_ENDPOINT="${CODEX_CLOUD_ENDPOINT:-https://api.openai.com}"
export CODEX_API_KEY="${CODEX_API_KEY:-}"
export CODEX_ORGANIZATION="${CODEX_ORGANIZATION:-}"

echo "🔧 Cloud Codex Configuration:"
echo "  Mode: $CODEX_MODE"
echo "  Resonance: $CODEX_RESONANCE"
echo "  Endpoint: $CODEX_CLOUD_ENDPOINT"
echo "  API Key: ${CODEX_API_KEY:+[SET]}${CODEX_API_KEY:-[NOT SET]}"
echo "  Organization: ${CODEX_ORGANIZATION:-[NOT SET]}"

# Validate required cloud settings
if [[ -z "${CODEX_API_KEY:-}" ]]; then
    echo "⚠️  Warning: CODEX_API_KEY not set. Cloud features may not work."
    echo "   Set CODEX_API_KEY in your .env file or environment"
fi

# Test cloud connectivity
echo "🌐 Testing cloud connectivity..."
if command -v curl >/dev/null 2>&1; then
    if curl -s --connect-timeout 5 "$CODEX_CLOUD_ENDPOINT" >/dev/null 2>&1; then
        echo "✅ Cloud endpoint reachable"
    else
        echo "⚠️  Warning: Cannot reach cloud endpoint"
    fi
else
    echo "ℹ️  curl not available, skipping connectivity test"
fi

# Initialize cloud components
echo "🏗️  Initializing cloud Codex components..."

# Ensure we have the necessary tools for cloud integration
echo "🔍 Checking cloud integration dependencies..."

# Check for required Node.js packages
cd codex-cli
if [[ ! -f "package.json" ]]; then
    echo "Error: codex-cli package.json not found"
    exit 1
fi

echo "📦 Installing cloud dependencies..."
npm install --legacy-peer-deps

# Check for cloud-specific Rust features
cd ../codex-rs
echo "🦀 Checking Rust cloud features..."
if cargo build --features="cloud" --dry-run >/dev/null 2>&1; then
    echo "✅ Cloud features available in Rust components"
else
    echo "ℹ️  Building with default features (cloud features may not be available)"
fi
cd ..

echo "✅ Cloud Codex setup completed!"
echo "🎯 Ready for cloud-based development in full resonance mode"
echo ""
echo "Environment variables summary:"
echo "  CODEX_MODE=$CODEX_MODE"
echo "  CODEX_RESONANCE=$CODEX_RESONANCE"
echo "  CODEX_CLOUD_ENDPOINT=$CODEX_CLOUD_ENDPOINT"
echo ""
echo "Next steps:"
echo "  - Ensure CODEX_API_KEY is set in your .env file"
echo "  - Run 'pnpm turbo run build' to build all packages"
echo "  - Use './launch_everything.sh' to start the full system"