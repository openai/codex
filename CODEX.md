# Codex Project Context

This document provides pre-loaded context for Codex CLI to avoid rebuilding context on each startup.

## GitHub CLI Integration Reference

The following commands have been pre-executed to provide context about the GitHub CLI integration:

```bash
# Search for "issue list" in GitHub CLI integration code
$ grep -R "issue list" -n codex-cli/src/utils/github/gh-cli.ts
# No results found (exit code: 1)

# Examine beginning of GitHub CLI integration file
$ sed -n '1,200p' ./codex-cli/src/utils/github/gh-cli.ts
import type { ExecResult } from "../agent/sandbox/interface.js";
import { exec } from "../agent/exec.js";
import { SandboxType } from "../agent/sandbox/interface.js";

... (152 more lines)

# Examine middle portion of GitHub CLI integration file
$ sed -n '200,350p' ./codex-cli/src/utils/github/gh-cli.ts
# Content of lines 200-350

# Examine approvals.ts to understand GitHub command handling
$ sed -n '340,420p' codex-cli/src/approvals.ts
        group: "Searching",
      };
    case "head":
      return {
... (78 more lines)

# Examine more of approvals.ts
$ sed -n '420,480p' codex-cli/src/approvals.ts
                group: "GitHub",
              };
          }
        default:
... (58 more lines)

# Repository structure
$ ls -a
.
..
.git
.github
... (18 more lines)

# GitHub CLI version
$ gh --version
gh version 2.69.0 (2025-03-19)
https://github.com/cli/cli/releases/tag/v2.69.0
```

## GitHub CLI Usage Examples

```bash
# List issues
gh issue list

# View specific issue
gh issue view 5

# Create issue
gh issue create --title "Title" --body "Description" 

# List pull requests
gh pr list
```

## Common Operations

1. To search for code: Use grep/GlobTool instead of manually running grep
2. To view GitHub issues: Use gh CLI commands directly
3. To create new experimental features: Create issue with experimental label

## Notes for Development

- GitHub CLI integration is experimental
- OpenTelemetry integration is being considered (issue #5)
- Heideggerian temporality research is tagged with bounty (issue #6)