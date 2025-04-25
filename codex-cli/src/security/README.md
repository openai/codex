# Adversys Cyber Agent

A specialized security mode for Codex CLI that provides offensive security capabilities.

## Getting Started

To use the security mode, run Codex with the `--security-mode` flag:

```bash
codex --security-mode "Scan example.com"
```

For more specific operations, you can add target and session parameters:

```bash
codex --security-mode "Perform security testing" --target example.com --session project-alpha
```

## Features

- Security tool detection and management
- Session-based testing with history tracking
- Interactive terminal for running security commands
- Persistent storage of security sessions and command history

## Available Commands

Once in the security mode, you'll see the `adversys>` prompt. Here are some available commands:

- `help` - Display available commands
- `tools` - List detected security tools
- `sessions` - List active security sessions
- `scan <target>` - Run a basic security scan
- `exit` - Exit the security mode

You can also run any standard command or security tool directly.

## Security Tools

Adversys automatically detects common security tools installed on your system, including:
- nmap
- sqlmap
- nikto
- gobuster
- hydra

## Sessions

Security sessions are stored in `~/.adversys/sessions/` and include:
- Session metadata (name, target, timestamps)
- Command history with outputs

## Future Improvements

- SQLite integration for more robust session storage
- Enhanced security tool integration
- Improved reporting with vulnerability findings
- Better OpenAI prompting for security-specific guidance 