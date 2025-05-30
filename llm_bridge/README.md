# LLM Bridge: Claude ↔ Codex Conversation System

This system enables automated conversation between Claude Code (Anthropic) and Codex CLI (OpenAI) through a file-based message bridge.

## Architecture

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Claude    │    │   Bridge    │    │   Codex     │
│ Interface   │◄──►│   Script    │◄──►│ Interface   │
│             │    │             │    │             │
│ (Terminal 2)│    │ (Terminal 1)│    │ (Terminal 3)│
└─────────────┘    └─────────────┘    └─────────────┘
```

## Files

- **`bridge.js`**: Central orchestrator that manages conversation flow
- **`claude_interface.js`**: Interface for Claude Code operator
- **`codex_interface.js`**: Interface that connects to Codex CLI
- **`setup.sh`**: Setup script to make everything executable

## How It Works

1. **Bridge Script** monitors message files and manages turn-taking
2. **Claude Interface** allows you to send messages that get forwarded to Codex
3. **Codex Interface** automatically runs Codex CLI with incoming messages
4. **Responses** flow back through the bridge to create a conversation loop

## Setup Instructions

1. **Run setup:**
   ```bash
   cd /mnt/c/Users/chris/codex/llm_bridge
   chmod +x setup.sh
   ./setup.sh
   ```

2. **Start the system in 3 terminals:**

   **Terminal 1 (Bridge):**
   ```bash
   node bridge.js
   ```

   **Terminal 2 (Claude):**
   ```bash
   node claude_interface.js
   ```

   **Terminal 3 (Codex):**
   ```bash
   export OPENAI_API_KEY="your_openai_key_here"
   node codex_interface.js
   ```

## Conversation Flow

1. Bridge starts and waits for Claude
2. You type a message in Claude Interface
3. Bridge forwards message to Codex Interface
4. Codex Interface runs `codex --quiet "your_message"`
5. Codex response gets sent back to Claude Interface
6. Process repeats for continuous conversation

## Example Conversation Starters

- "Hello, I'm Claude Code. What can you tell me about yourself?"
- "Let's collaborate on building a simple web application together"
- "I'd like to understand how you approach problem-solving"
- "Can you help me create a Python script, and I'll help you improve it?"

## Features

- **Turn Management**: Prevents message collision
- **Full Logging**: All conversations logged to `conversation_log.txt`
- **Error Handling**: Graceful handling of Codex CLI errors
- **Clean Interface**: Clear separation of Claude and Codex messages

## Files Created During Operation

- `claude_to_codex.txt`: Messages from Claude to Codex
- `codex_to_claude.txt`: Messages from Codex to Claude
- `conversation_log.txt`: Full conversation history
- `turn_control.txt`: Current speaker indicator
- `bridge_status.txt`: Bridge status information

## Stopping the System

- Type "quit" in Claude Interface
- Press Ctrl+C in any terminal to stop that component
- Bridge will log all activity and can be restarted anytime