#!/bin/bash

# LLM Bridge Setup Script

echo "Setting up LLM Bridge for Claude ↔ Codex conversation..."

# Make scripts executable
chmod +x bridge.js
chmod +x claude_interface.js
chmod +x codex_interface.js

# Create bridge directory if it doesn't exist
mkdir -p /mnt/c/Users/chris/codex/llm_bridge

echo "✅ Bridge scripts are ready!"
echo ""
echo "To start the LLM conversation:"
echo ""
echo "Terminal 1 (Bridge): node bridge.js"
echo "Terminal 2 (Claude):  node claude_interface.js"
echo "Terminal 3 (Codex):   OPENAI_API_KEY=your_key node codex_interface.js"
echo ""
echo "Make sure to set your OPENAI_API_KEY in Terminal 3!"
echo ""
echo "The conversation flow:"
echo "1. Bridge coordinates turns"
echo "2. Claude interface lets you send messages to Codex"
echo "3. Codex interface automatically processes with Codex CLI"
echo "4. Responses flow back through the bridge"