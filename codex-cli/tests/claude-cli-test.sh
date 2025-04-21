#!/bin/bash
# Test the Claude provider in the CLI showing complete output

# Set environment variables
export DEBUG=1
export CODEX_DEFAULT_PROVIDER=claude

# Run with Claude model
echo "==== Testing Claude Provider in CLI ===="
echo "Running: node dist/cli.js -q -m claude-3-sonnet-20240229 \"Say hello and tell me today's date\""
echo ""
echo "OUTPUT:"
node dist/cli.js -q -m claude-3-sonnet-20240229 "Say hello and tell me today's date" | tee >(cat >&2)
echo ""
echo "==== Test Complete ===="