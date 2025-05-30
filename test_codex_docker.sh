#!/bin/bash
set -e

echo "🚀 Testing Codex CLI in Docker..."
echo "================================"

# Load environment variables
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
    echo "✅ Loaded API key from .env"
else
    echo "❌ No .env file found"
    exit 1
fi

# Change to codex-cli directory
cd codex-cli

# Test 1: Simple command that doesn't require internet
echo "🧪 Test 1: Basic Docker container functionality"
./scripts/run_in_container.sh "echo 'Docker container is working'" || {
    echo "❌ Docker container test failed"
    exit 1
}
echo "✅ Docker container working"

# Test 2: Simple coding task
echo "🧪 Test 2: Simple Python script generation and execution"
./scripts/run_in_container.sh "Create a simple Python script that prints 'Hello from Codex in Docker!' and run it" || {
    echo "❌ Codex coding test failed"
    exit 1
}
echo "✅ Codex coding task completed"

echo "🎉 All tests passed! Codex Docker setup is working correctly."