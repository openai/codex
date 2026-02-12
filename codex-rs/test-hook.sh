#!/bin/bash
# Test hook script - logs hook events to a file
# Usage: Called by Codex with JSON payload as the last argument

HOOK_LOG="/tmp/codex-hook-test.log"

# Get timestamp
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')

# The JSON payload is the last argument
PAYLOAD="${@: -1}"

# Extract event type from JSON (simple grep approach)
EVENT_TYPE=$(echo "$PAYLOAD" | grep -o '"hook_event":{[^}]*"[^"]*":' | head -1 | sed 's/.*"\([^"]*\)":$/\1/')

# Log the event
echo "[$TIMESTAMP] Hook fired: $EVENT_TYPE" >> "$HOOK_LOG"
echo "Payload: $PAYLOAD" >> "$HOOK_LOG"
echo "---" >> "$HOOK_LOG"

# Exit successfully
exit 0
