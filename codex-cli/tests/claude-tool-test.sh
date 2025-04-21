#!/bin/bash
# Test the Claude provider with tool calls showing complete output

# Set environment variables
export DEBUG=1
export CODEX_DEFAULT_PROVIDER=claude

# Run with Claude model and a command that requires tools
echo "==== Testing Claude Provider with Tool Calls ===="
echo "Running: node dist/cli.js -q -m claude-3-sonnet-20240229 \"Run ls command to show the files in the current directory\""
echo ""
echo "OUTPUT:"
node dist/cli.js -q -m claude-3-sonnet-20240229 "Run ls command to show the files in the current directory" | tee >(cat >&2)
echo ""
echo "==== Test Complete ===="