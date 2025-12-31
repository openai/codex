# tweakcc System Prompt Management

> Downloading, Editing, and Syncing System Prompts

## Overview

tweakcc allows you to customize Claude Code's system prompts - the instructions that define how Claude behaves. System prompts are stored as markdown files that you can edit directly.

## System Prompt Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          System Prompt Flow                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  GitHub Repository              Local Cache                 User Files  │
│  ┌─────────────┐               ┌─────────────┐             ┌──────────┐ │
│  │prompts-X.Y.Z│──download────>│prompt-data- │──sync──────>│system-   │ │
│  │.json        │               │cache/       │             │prompts/  │ │
│  └─────────────┘               └─────────────┘             └──────────┘ │
│                                                                   │      │
│                                                                   ▼      │
│                                                             ┌──────────┐ │
│                                                             │User edits│ │
│                                                             │.md files │ │
│                                                             └──────────┘ │
│                                                                   │      │
│                                                                   ▼      │
│  ┌─────────────┐               ┌─────────────┐             ┌──────────┐ │
│  │Claude Code  │<──patch───────│Patch Engine │<──load──────│Markdown  │ │
│  │cli.js       │               │             │             │Prompts   │ │
│  └─────────────┘               └─────────────┘             └──────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Prompt Data Format

### StringsFile (JSON)

Downloaded from GitHub, one file per Claude Code version:

```json
{
  "version": "2.0.76",
  "prompts": [
    {
      "name": "System Prompt",
      "id": "system-prompt",
      "description": "Main system instructions for Claude",
      "pieces": [
        "You are Claude, an AI assistant...",
        "} your response. ${",
        "} other instructions..."
      ],
      "identifiers": [0, 1, 2],
      "identifierMap": {
        "0": "SETTINGS",
        "1": "FORMAT_HELPER",
        "2": "TOOL_LIST"
      },
      "version": "2.0.70"
    }
  ]
}
```

### Field Descriptions

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Human-readable prompt name |
| `id` | string | Unique identifier (used for filename) |
| `description` | string | Brief description of purpose |
| `pieces` | string[] | Text fragments split at variable boundaries |
| `identifiers` | number[] | Indices into identifierMap for each boundary |
| `identifierMap` | Record<string, string> | Maps indices to variable names |
| `version` | string | CC version when prompt was last changed |

### Piece/Identifier Reconstruction

```
pieces:      ["Start ", " middle ", " end"]
identifiers: [0,        1        ]
identifierMap: {0: "VAR1", 1: "VAR2"}

Reconstructed: "Start ${VAR1} middle ${VAR2} end"
```

## Markdown Format

### System Prompt Markdown

Prompts are stored as markdown with YAML frontmatter using HTML comment delimiters:

```markdown
<!--
name: System Prompt
description: Main system instructions for Claude
ccVersion: 2.0.76
variables:
  - SETTINGS
  - FORMAT_HELPER
  - TOOL_LIST
-->

You are Claude, an AI assistant made by Anthropic.

Your preferred name is ${SETTINGS.preferredName}.

Available tools: ${TOOL_LIST}

When responding, use ${FORMAT_HELPER} for formatting.
```

### Frontmatter Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name |
| `description` | string | Yes | Brief description |
| `ccVersion` | string | Yes | Claude Code version |
| `variables` | string[] | No | List of variable placeholders |

### Variable Syntax

Variables use the `${VARIABLE_NAME}` syntax:

```markdown
Your name is ${SETTINGS.preferredName}.
Available tools are: ${AVAILABLE_TOOLS_LIST}
```

**Special Variables:**
- `${SETTINGS.preferredName}` - User's preferred name
- `${AVAILABLE_TOOLS_LIST}` - List of enabled tools
- `${BASH_TOOL_NAME}` - Name of bash tool
- `${READ_TOOL_NAME}` - Name of read tool
- `${WRITE_TOOL_NAME}` - Name of write tool

## Sync Process

### Initial Sync

On first run for a Claude Code version:

1. Download `prompts-{version}.json` from GitHub
2. Cache locally in `prompt-data-cache/`
3. Parse JSON into prompt objects
4. Reconstruct full prompts from pieces + identifiers
5. Create markdown files in `system-prompts/`
6. Record original hashes

### Subsequent Syncs

On each startup:

1. Get current Claude Code version
2. Check if cached JSON exists
3. If not, download and cache
4. For each prompt:
   - Check if markdown file exists
   - Compare hashes (original vs applied vs current)
   - Apply appropriate action

### Sync Actions

| Original | Applied | Current | Action |
|----------|---------|---------|--------|
| A | A | A | Skip (unchanged) |
| A | A | B | Keep user changes |
| A | B | B | Skip (already applied) |
| B | A | A | Update (upstream changed) |
| B | A | B | Skip (user has new version) |
| A | B | C | **CONFLICT** - generate diff |

## Conflict Resolution

### Conflict Detection

A conflict occurs when:
1. User has modified the prompt (current != applied)
2. Anthropic has also updated the prompt (original changed)

### Conflict Handling

```
┌─────────────────────────────────────────────────────────────────┐
│                      Conflict Detected                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  User's version:     "You are Claude, a helpful AI..."         │
│  Anthropic's new:    "You are Claude, an AI assistant..."      │
│                                                                  │
│  Action: Generate HTML diff file                                │
│                                                                  │
│  Output: system-prompts/system-prompt.conflict.html             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### HTML Diff Format

Generated conflict files show side-by-side comparison:

```html
<!DOCTYPE html>
<html>
<head>
  <title>Prompt Conflict: system-prompt</title>
  <style>
    .diff-container { display: flex; }
    .diff-left { background: #fdd; }
    .diff-right { background: #dfd; }
    .changed { background: #ff0; }
  </style>
</head>
<body>
  <h1>Conflict in: system-prompt</h1>
  <div class="diff-container">
    <div class="diff-left">
      <h2>Your Version</h2>
      <pre>Your customized content...</pre>
    </div>
    <div class="diff-right">
      <h2>New Anthropic Version</h2>
      <pre>Updated content from Anthropic...</pre>
    </div>
  </div>
</body>
</html>
```

## Hash Index Files

### systemPromptOriginalHashes.json

Stores baseline hashes from downloaded JSON:

```json
{
  "system-prompt-2.0.76": "a1b2c3d4e5f6...",
  "tool-bash-2.0.76": "f6e5d4c3b2a1...",
  "agent-explore-2.0.70": "1a2b3c4d5e6f..."
}
```

### systemPromptAppliedHashes.json

Stores hashes when prompts were last applied:

```json
{
  "system-prompt-2.0.76": "a1b2c3d4e5f6...",
  "tool-bash-2.0.76": "modified123..."
}
```

### Hash Key Format

```
{promptId}-{ccVersion}
```

Example: `system-prompt-2.0.76`

## Prompt Categories

### Core Prompts

| ID | Name | Description |
|----|------|-------------|
| `system-prompt` | System Prompt | Main Claude instructions |
| `system-prompt-mcp-cli` | MCP CLI Prompt | MCP command line interface |
| `compact-prompt` | Compact Prompt | Conversation compression |

### Tool Prompts

| ID | Name | Description |
|----|------|-------------|
| `tool-bash` | Bash Tool | Shell command execution |
| `tool-read` | Read Tool | File reading |
| `tool-write` | Write Tool | File writing |
| `tool-edit` | Edit Tool | File editing |
| `tool-glob` | Glob Tool | File pattern matching |
| `tool-grep` | Grep Tool | Text search |
| `tool-websearch` | Web Search | Web searching |
| `tool-webfetch` | Web Fetch | Web page fetching |

### Agent Prompts

| ID | Name | Description |
|----|------|-------------|
| `agent-explore` | Explore Agent | Codebase exploration |
| `agent-plan` | Plan Agent | Implementation planning |
| `agent-task` | Task Agent | Task execution |

### Utility Prompts

| ID | Name | Description |
|----|------|-------------|
| `context-warning` | Context Warning | Context length warning |
| `quota-warning` | Quota Warning | API quota warning |
| `error-analysis` | Error Analysis | Error handling prompts |

## API Reference

### Loading Prompts

```typescript
import { loadSystemPromptsWithRegex } from './systemPromptSync';

const prompts = await loadSystemPromptsWithRegex(
  '2.0.76',        // version
  false,           // shouldEscapeNonAscii
  '2025-12-21'     // buildTime
);

// Returns Map<string, SystemPromptEntry>
for (const [id, entry] of prompts) {
  console.log(id, entry.regex, entry.replacement);
}
```

### Downloading Prompts

```typescript
import { downloadStringsFile } from './systemPromptDownload';

const stringsFile = await downloadStringsFile('2.0.76');
// Returns StringsFile with version and prompts array
```

### Syncing Prompts

```typescript
import { syncSystemPrompts } from './systemPromptSync';

const summary = await syncSystemPrompts('2.0.76');
// Returns SyncSummary with results for each prompt
```

### Parsing Markdown

```typescript
import { parseMarkdownPrompt } from './systemPromptSync';

const markdown = `<!--
name: Test Prompt
description: A test
ccVersion: 2.0.76
variables:
  - VAR1
-->

Content with ${VAR1}`;

const parsed = parseMarkdownPrompt(markdown);
// {
//   name: "Test Prompt",
//   description: "A test",
//   ccVersion: "2.0.76",
//   variables: ["VAR1"],
//   content: "Content with ${VAR1}",
//   contentLineOffset: 8
// }
```

### Checking for Changes

```typescript
import { hasUnappliedSystemPromptChanges } from './systemPromptHashIndex';

const hasChanges = await hasUnappliedSystemPromptChanges(
  '/path/to/system-prompts'
);
// Returns true if prompts were modified since last apply
```

## Prompt File Examples

### Tool Prompt Example

```markdown
<!--
name: Bash Tool
description: Instructions for executing shell commands
ccVersion: 2.0.76
variables:
  - SANDBOX_MODE
  - ALLOWED_COMMANDS
-->

## Bash Tool

Execute shell commands in a sandboxed environment.

### Sandbox Mode
Current mode: ${SANDBOX_MODE}

### Allowed Commands
${ALLOWED_COMMANDS}

### Guidelines
- Always quote file paths with spaces
- Use absolute paths when possible
- Check command success with $?
```

### Agent Prompt Example

```markdown
<!--
name: Explore Agent
description: Specialized agent for codebase exploration
ccVersion: 2.0.70
variables:
  - SEARCH_TOOLS
  - EXPLORATION_GUIDELINES
-->

# Explore Agent

You are a specialized agent for exploring codebases.

## Available Tools
${SEARCH_TOOLS}

## Guidelines
${EXPLORATION_GUIDELINES}

## Workflow
1. Understand the search goal
2. Use appropriate tools
3. Synthesize findings
4. Report results
```

## Editing Tips

### Safe Editing

1. **Keep Variables:** Don't remove `${VARIABLE}` placeholders
2. **Preserve Structure:** Maintain markdown formatting
3. **Test Changes:** Apply and verify in Claude Code

### Common Customizations

**Add custom instructions:**
```markdown
## My Custom Rules
- Always use TypeScript
- Prefer functional programming
- Add comments to complex code
```

**Modify tool behavior:**
```markdown
### Additional Guidelines
- Run tests after file changes
- Create backups before editing
```

### Reverting Changes

To revert a prompt to original:

1. Delete the `.md` file from `system-prompts/`
2. Run `tweakcc` - it will recreate from cache
3. Apply customizations

Or use "Restore original Claude Code" in main menu.

## Troubleshooting

### Prompt Not Applying

**Symptoms:** Changes in markdown not reflected in Claude Code

**Causes:**
1. Variable syntax error (`${VAR}` vs `$VAR`)
2. Frontmatter parsing error
3. Hash mismatch

**Solution:**
1. Check variable names match identifierMap
2. Validate frontmatter YAML
3. Delete hash files and re-sync

### Sync Errors

**429 Rate Limit:**
```
Error: Rate limited by GitHub (429)
```
Wait a few minutes and retry.

**404 Not Found:**
```
Error: Prompt file not found for version 2.0.XX
```
Version may be too new. Wait for tweakcc update.

### Conflict Not Resolving

1. Open generated `.conflict.html` file
2. Manually merge changes in `.md` file
3. Delete `.conflict.html` file
4. Re-apply customizations
