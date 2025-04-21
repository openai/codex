#!/bin/bash
# Simple test to verify Claude response is displayed

# Set environment variables
export CODEX_DEFAULT_PROVIDER=claude
export DEBUG=1

# Run with -q flag to print output to console
echo "Testing Claude provider output:"
node dist/cli.js -q -m claude-3-sonnet-20240229 "Tell me a fact about cats"