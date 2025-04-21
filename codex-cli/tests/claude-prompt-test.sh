#!/bin/bash
# Test sending a real prompt to Claude

# Set environment variables
export DEBUG=1
export CODEX_DEFAULT_PROVIDER=claude
# This test assumes CLAUDE_API_KEY is already set in environment

# Run a simple prompt in quiet mode
echo "==== Testing real Claude prompt ===="
node dist/cli.js -q -m claude-3-sonnet-20240229 "Write a haiku about coding"

# Exit with success
exit 0