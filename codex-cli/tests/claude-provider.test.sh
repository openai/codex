#!/bin/bash
# Test script for Claude provider integration

# Use Claude by default
export CODEX_DEFAULT_PROVIDER=claude

# Set Claude API key from environment
# This assumes you've already set CLAUDE_API_KEY or ANTHROPIC_API_KEY in your environment

# Run Codex with Claude
echo "Running Codex with Claude provider..."
node dist/cli.js -q "Hello from Claude test"

# Use Claude model explicitly
echo "Running Codex with explicit Claude model..."
node dist/cli.js -q -m claude-3-sonnet-20240229 "Hello from Claude with explicit model"

# Exit with success
exit 0