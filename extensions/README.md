# Codex Sub-Agents & Deep Research - VS Code Extension

VS Code extension for Codex multi-agent delegation and deep research capabilities.

## Features

### 🤖 Sub-Agent Delegation

Delegate tasks to specialized agents:
- **Test Generator**: Auto-generate unit tests
- **Security Auditor**: CVE scan and vulnerability detection
- **Deep Researcher**: Multi-level research with citations
- **Code Reviewer**: Automated code review and suggestions

### 🔍 Deep Research

Execute deep research directly from VS Code:
- Multi-depth exploration
- Citation tracking
- Contradiction detection
- Lightweight fallback mode

### 📊 Agent Status Monitoring

Real-time monitoring of:
- Active agents
- Task progress
- Token usage
- Generated artifacts

## Commands

| Command | Description |
|---------|-------------|
| `Codex: Delegate to Sub-Agent` | Delegate task to a sub-agent |
| `Codex: Deep Research` | Execute deep research on a topic |
| `Codex: List Available Agents` | Show all configured agents |
| `Codex: Review Code` | Review current file with Code Reviewer agent |

## Requirements

- Codex CLI installed and configured
- `.codex/agents/` directory with agent definitions
- Optional: Slack webhook for notifications
- Optional: GitHub token for PR automation

## Extension Settings

This extension contributes the following settings:

* `codex.agentsPath`: Path to agent definitions (default: `.codex/agents`)
* `codex.defaultBudget`: Default token budget (default: 40000)
* `codex.slackWebhook`: Slack webhook URL for notifications
* `codex.githubToken`: GitHub token for PR automation

## Usage

### 1. Delegate to Agent

1. Open Command Palette (`Ctrl+Shift+P` or `Cmd+Shift+P`)
2. Run `Codex: Delegate to Sub-Agent`
3. Select agent from list
4. Enter task goal
5. Monitor progress in terminal

### 2. Deep Research

1. Open Command Palette
2. Run `Codex: Deep Research`
3. Enter research topic
4. Select depth (1-5)
5. View results in `artifacts/report.md`

### 3. Code Review

1. Open file to review
2. Run `Codex: Review Code`
3. View review comments in `review-comments/`

## Architecture

```
VS Code Extension
    ↓
Codex CLI (delegate/research commands)
    ↓
Agent Runtime (Rust)
    ↓
[Test Gen | Security | Researcher | Reviewer]
    ↓
Artifacts + Reports
    ↓
[GitHub PR | Slack Notification | Webhook]
```

## Development

```bash
# Install dependencies
npm install

# Compile
npm run compile

# Watch mode
npm run watch

# Package extension
vsce package
```

## Release Notes

### 0.1.0

Initial release:
- Sub-agent delegation
- Deep research
- Agent status monitoring
- GitHub/Slack integration
- Webhook support

## License

MIT

## Author

zapabob / Ryo Minegishi

