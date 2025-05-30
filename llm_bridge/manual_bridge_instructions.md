# Manual Bridge Instructions for Interactive Codex

Since Codex is running in full interactive mode with a terminal UI, we'll use a **manual bridge approach** where you act as the intermediary.

## Current Situation
- **Codex**: Running in interactive mode in WSL session (Terminal UI with prompts)
- **Claude Code**: Running here (can read/write files to coordinate)
- **You**: Act as the bridge between us

## Manual Bridge Process

### 1. Start the Coordination System

First, let me set up a simple coordination file system:

```bash
# In this terminal
node /mnt/c/Users/chris/codex/llm_bridge/manual_bridge.js
```

### 2. Conversation Flow

**Step A:** I'll write my message to a file
**Step B:** You copy that message and paste it into the Codex terminal
**Step C:** You copy Codex's response and paste it back to me
**Step D:** I'll process the response and write the next message

### 3. File Locations

- **My messages to Codex**: `/mnt/c/Users/chris/codex/llm_bridge/claude_says.txt`
- **Your input of Codex responses**: You'll paste them when I prompt you

## Advantages of Manual Bridge

1. **Full Control**: You can see exactly what both AIs are saying
2. **No Technical Issues**: Works with Codex's interactive UI
3. **Better Observation**: You can observe the conversation dynamics
4. **Intervention Possible**: You can steer the conversation if needed

## Ready to Start?

Just say "start" and I'll:
1. Write my first message to `claude_says.txt` 
2. Ask you to copy it to Codex
3. Wait for you to paste back Codex's response
4. Continue the conversation loop

Would you like to proceed with this manual bridge approach?