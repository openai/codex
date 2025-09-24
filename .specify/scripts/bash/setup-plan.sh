#!/bin/bash

# Get the latest spec file
SPECS_DIR="/home/irichard/dev/study/codex-study/docs/specs"
FEATURE_SPEC=$(ls -t "$SPECS_DIR"/*.md 2>/dev/null | head -1)

if [ -z "$FEATURE_SPEC" ]; then
    echo "Error: No specification file found"
    exit 1
fi

# Get current branch
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")

# Create implementation plan file
IMPL_PLAN="$SPECS_DIR/implementation-plan.md"
touch "$IMPL_PLAN"

# Output JSON
if [ "$1" = "--json" ]; then
    echo "{\"FEATURE_SPEC\":\"$FEATURE_SPEC\",\"IMPL_PLAN\":\"$IMPL_PLAN\",\"SPECS_DIR\":\"$SPECS_DIR\",\"BRANCH\":\"$BRANCH\"}"
fi