# Git and GitHub CLI Approval Configuration Guide

## Overview

Codex provides fine-grained control over approval requirements for Git and GitHub CLI commands. This feature allows you to:

- Define which Git and GitHub CLI commands are automatically approved
- Specify commands that always require explicit approval
- Set default approval behavior for each tool
- Override settings via command-line flags

## Configuration Options

You can configure Git and GitHub CLI approval settings in your `~/.codex/config.json` file:

```json
{
  "git": {
    "requireApprovalByDefault": true,
    "autoApprovedCommands": [
      "status",
      "log",
      "diff",
      "branch",
      "show"
    ],
    "requireApprovalCommands": [
      "commit",
      "push",
      "merge",
      "rebase",
      "reset",
      "checkout"
    ]
  },
  "githubCli": {
    "requireApprovalByDefault": false,
    "autoApprovedCommands": [
      "issue list",
      "issue view",
      "pr list",
      "pr view",
      "workflow list",
      "workflow view"
    ],
    "requireApprovalCommands": [
      "pr create",
      "pr merge",
      "issue create",
      "issue close"
    ]
  }
}
```

### Configuration Fields Explained

#### Git Configuration

- `requireApprovalByDefault`: When `true`, all Git commands require approval unless explicitly allowed. When `false`, commands are auto-approved unless explicitly denied. Default is `true` for safety.

- `autoApprovedCommands`: List of Git subcommands that are always auto-approved regardless of the default setting. Default includes safe read-only commands like `status`, `log`, `diff`, `branch`, and `show`.

- `requireApprovalCommands`: List of Git subcommands that always require approval regardless of the default setting. Default includes potentially destructive commands like `commit`, `push`, `merge`, `rebase`, `reset`, and `checkout`.

#### GitHub CLI Configuration

- `requireApprovalByDefault`: When `true`, all GitHub CLI commands require approval unless explicitly allowed. When `false`, commands are auto-approved unless explicitly denied. Default is `false` to maintain compatibility with existing behavior.

- `autoApprovedCommands`: List of GitHub CLI subcommands that are always auto-approved regardless of the default setting. Default includes safe read-only commands like `issue list`, `pr list`, and various view operations.

- `requireApprovalCommands`: List of GitHub CLI subcommands that always require approval regardless of the default setting. Default includes potentially destructive operations like `pr create`, `pr merge`, `issue create`, and `issue close`.

## Command-Line Flags

For quick configuration without modifying your config file, you can use these command-line flags:

```bash
# Always prompt for Git commands
codex --git-approval=prompt "your prompt here"

# Auto-approve Git commands (unless explicitly denied)
codex --git-approval=auto "your prompt here"

# Always prompt for GitHub CLI commands
codex --github-approval=prompt "your prompt here"

# Auto-approve GitHub CLI commands (unless explicitly denied)
codex --github-approval=auto "your prompt here"
```

## Usage Examples

### Example 1: Default Behavior

With default settings:
- Git commands like `status`, `log`, `diff` will be auto-approved
- Git commands like `commit`, `push`, `merge` will require approval
- Any other Git command will require approval (default is `requireApprovalByDefault: true`)
- GitHub CLI commands like `issue list`, `pr view` will be auto-approved
- GitHub CLI commands like `pr create`, `issue close` will require approval
- Any other GitHub CLI command will be auto-approved (default is `requireApprovalByDefault: false`)

### Example 2: Custom Configuration

If you want to:
- Auto-approve all Git commands except potentially destructive ones
- Require approval for all GitHub CLI commands

```json
{
  "git": {
    "requireApprovalByDefault": false,
    "requireApprovalCommands": [
      "commit", "push", "merge", "rebase", "reset", "checkout",
      "branch -D", "clean", "stash"
    ]
  },
  "githubCli": {
    "requireApprovalByDefault": true,
    "autoApprovedCommands": [
      "issue list", "issue view", "pr list", "pr view"
    ]
  }
}
```

### Example 3: Command-Line Override

To temporarily change approval behavior for a single session:

```bash
# Ask for prompt on all git commands for this session only
codex --git-approval=prompt "Clone the repo and analyze the code structure"

# Auto-approve GitHub CLI commands for this session only
codex --github-approval=auto "Create a PR for my feature branch"
```

## Safety Considerations

- By default, Git commands require approval (safer)
- GitHub CLI commands are auto-approved by default to maintain compatibility with existing behavior
- When in doubt about a command's safety, prefer `requireApprovalByDefault: true`
- Commands that modify state should typically be in `requireApprovalCommands`
- Read-only commands are good candidates for `autoApprovedCommands`

## Recommended Workflow

1. Start with the default configuration
2. Add commonly used safe commands to `autoApprovedCommands` 
3. Add potentially destructive commands to `requireApprovalCommands`
4. Use command-line flags for temporary overrides

## Troubleshooting

- If commands are being auto-approved unexpectedly, check if `requireApprovalByDefault` is `false`
- If commands are requiring approval unexpectedly, check if they're listed in `requireApprovalCommands`
- Command-line flags override configuration file settings for the current session
- Multi-word commands for GitHub CLI (e.g., `issue list`) must be entered exactly as shown

By configuring these settings appropriately, you can balance convenience and safety when working with Git and GitHub CLI commands in Codex.