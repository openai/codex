#!/bin/bash

# Set up environment
export OLLAMA_API_KEY="dummy"

# Path to the local codex
CODEX_PATH="/Volumes/Untitled/coder/gemini-cli/codex/codex-cli/bin/codex.js"

# Run codex with all arguments passed to this script
exec node "$CODEX_PATH" "$@"