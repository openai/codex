#!/usr/bin/env bash
# Require OpenAI API key
if [ -z "$OPENAI_API_KEY" ]; then
  echo "Error: OPENAI_API_KEY environment variable is not set"
  exit 1
fi
set -e

# Build and start the Codex server
echo "Building Codex server..."
cd codex-cli
npm install
npm run build

echo "Starting Codex server on port 3000..."
node dist/server.js &
SERVER_PID=$!

# Return to root and start the React interface
cd ..
echo "Starting CodexInterface..."
cd CodexInterface/codexinterface
npm install
npm run dev

# When the interface exits, stop the server
kill $SERVER_PID