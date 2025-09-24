#!/bin/bash

# Parse JSON argument
if [ "$1" = "--json" ] && [ -n "$2" ]; then
    FEATURE_DESC="$2"
else
    echo "Usage: $0 --json \"feature description\""
    exit 1
fi

# Generate branch name from feature description (first 50 chars, cleaned)
BRANCH_NAME=$(echo "$FEATURE_DESC" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g' | cut -c1-50 | sed 's/-$//')
BRANCH_NAME="feature/${BRANCH_NAME}"

# Generate spec file name
TIMESTAMP=$(date +%Y%m%d)
SPEC_FILE="/home/irichard/dev/study/codex-study/docs/specs/${TIMESTAMP}-complete-codex-chrome-implementation.md"

# Create and checkout branch
git checkout -b "$BRANCH_NAME" 2>/dev/null || git checkout "$BRANCH_NAME"

# Create spec directory if it doesn't exist
mkdir -p "$(dirname "$SPEC_FILE")"

# Touch the spec file
touch "$SPEC_FILE"

# Output JSON result
echo "{\"BRANCH_NAME\":\"$BRANCH_NAME\",\"SPEC_FILE\":\"$SPEC_FILE\"}"