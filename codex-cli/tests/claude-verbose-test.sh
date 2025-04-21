#!/bin/bash
# Test the Claude provider with full stdout

# Set environment variables for Claude
export DEBUG=1
export CODEX_DEFAULT_PROVIDER=claude
export PRETTY_PRINT=1
export CODEX_FULL_STDOUT=1

# Run with Claude model
echo "==== Testing Claude Provider with Visible Output ===="
echo "Running with command: node dist/cli.js -q --full-stdout -m claude-3-sonnet-20240229 \"What is 2+2?\""
echo ""
echo "OUTPUT:"
node dist/cli.js -q --full-stdout -m claude-3-sonnet-20240229 "What is 2+2?"

echo ""
echo "==== Test Complete ===="