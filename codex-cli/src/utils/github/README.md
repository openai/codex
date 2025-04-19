# GitHub CLI Integration

This module provides integration with GitHub CLI (`gh`) for Codex. It allows users to interact with GitHub directly from Codex to manage issues, pull requests, and workflows.

## Requirements

- GitHub CLI (`gh`) must be installed and authenticated on the system
- You can install GitHub CLI from: https://cli.github.com/

## Usage Examples

The GitHub CLI integration supports all standard `gh` commands:

### Issues

```
gh issue create "Bug title" "Bug description"
gh issue list
gh issue view 1
```

### Pull Requests

```
gh pr create "Feature title" "Feature description"
gh pr list
gh pr view 1
```

### Workflows

```
gh workflow list
gh workflow view workflow-name.yml
gh workflow run workflow-name.yml
```

## Implementation Details

The integration provides:

1. Utility functions for common GitHub operations
2. Sandbox-aware execution
3. Platform-independent installation check
4. Proper error handling

All GitHub commands are executed with the `SandboxType.NONE` setting to ensure proper functionality.