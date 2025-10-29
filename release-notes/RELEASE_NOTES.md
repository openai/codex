## @just-every/code v0.4.4

This release tightens chat handoffs, makes MCP configuration smoother, and tidies session management for better day-to-day UX.

### Changes
- TUI: Interrupts in-flight runs when `/new` starts a fresh chat so responses never bleed between sessions.
- TUI/MCP: Keeps the selected MCP row visible while scrolling large server lists.
- Agents: Refreshes the Enabled toggle UX and persists state immediately in history.
- Config: Surfaces legacy `~/.codex/prompts` directories so custom prompts load automatically.
- Rollout: Sorts session history by latest activity to make resume picks faster.

### Install
```bash
npm install -g @just-every/code@latest
code
```

Compare: https://github.com/just-every/code/compare/v0.4.3...v0.4.4
