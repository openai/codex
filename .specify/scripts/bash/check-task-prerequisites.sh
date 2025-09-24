#!/bin/bash

# Get feature directory
FEATURE_DIR="/home/irichard/dev/study/codex-study/docs/specs"

# List available documents
AVAILABLE_DOCS=()
[ -f "$FEATURE_DIR/implementation-plan.md" ] && AVAILABLE_DOCS+=("implementation-plan.md")
[ -f "$FEATURE_DIR/research.md" ] && AVAILABLE_DOCS+=("research.md")
[ -f "$FEATURE_DIR/data-model.md" ] && AVAILABLE_DOCS+=("data-model.md")
[ -d "$FEATURE_DIR/contracts" ] && AVAILABLE_DOCS+=("contracts/")
[ -f "$FEATURE_DIR/quickstart.md" ] && AVAILABLE_DOCS+=("quickstart.md")
[ -f "$FEATURE_DIR/tasks.md" ] && AVAILABLE_DOCS+=("tasks.md")

# Output JSON
if [ "$1" = "--json" ]; then
    DOCS_JSON=$(printf '"%s",' "${AVAILABLE_DOCS[@]}" | sed 's/,$//')
    echo "{\"FEATURE_DIR\":\"$FEATURE_DIR\",\"AVAILABLE_DOCS\":[$DOCS_JSON]}"
fi