# tweakcc Data Formats

> JSON Schemas and Data Structure Specifications

## Overview

tweakcc uses several data formats for configuration, prompt storage, and caching.

## Configuration JSON

### Location

```
~/.tweakcc/config.json
```

### Full Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "version": {
      "type": "string",
      "description": "tweakcc version that created this config",
      "example": "3.2.2"
    },
    "lastModified": {
      "type": "string",
      "format": "date-time",
      "description": "ISO timestamp of last modification"
    },
    "changesApplied": {
      "type": "boolean",
      "description": "Whether current changes have been applied"
    },
    "ccInstallationPath": {
      "type": "string",
      "description": "Explicit path to Claude Code installation"
    },
    "themeId": {
      "type": "string",
      "description": "ID of currently selected theme"
    },
    "themes": {
      "type": "array",
      "items": { "$ref": "#/definitions/Theme" }
    },
    "thinkingVerbs": { "$ref": "#/definitions/ThinkingVerbsConfig" },
    "thinkingStyle": { "$ref": "#/definitions/ThinkingStyleConfig" },
    "userMessageDisplay": { "$ref": "#/definitions/UserMessageDisplayConfig" },
    "inputBox": { "$ref": "#/definitions/InputBoxConfig" },
    "misc": { "$ref": "#/definitions/MiscConfig" },
    "toolsets": {
      "type": "array",
      "items": { "$ref": "#/definitions/Toolset" }
    },
    "selectedToolset": {
      "type": "string",
      "description": "Currently active toolset ID"
    }
  },
  "definitions": {
    "Theme": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "id": { "type": "string" },
        "colors": { "$ref": "#/definitions/ThemeColors" }
      },
      "required": ["name", "id", "colors"]
    },
    "ThemeColors": {
      "type": "object",
      "additionalProperties": { "type": "string" },
      "description": "62+ color properties"
    },
    "ThinkingVerbsConfig": {
      "type": "object",
      "properties": {
        "format": { "type": "string" },
        "verbs": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "ThinkingStyleConfig": {
      "type": "object",
      "properties": {
        "reverseMirror": { "type": "boolean" },
        "updateInterval": { "type": "number" },
        "phases": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "UserMessageDisplayConfig": {
      "type": "object",
      "properties": {
        "format": { "type": "string" },
        "styling": {
          "type": "array",
          "items": {
            "enum": ["bold", "italic", "underline", "strikethrough", "inverse"]
          }
        },
        "foregroundColor": { "type": "string" },
        "backgroundColor": { "type": ["string", "null"] },
        "borderStyle": {
          "enum": ["none", "single", "double", "round", "bold", "singleDouble", "doubleSingle", "classic", "arrow"]
        },
        "borderColor": { "type": "string" },
        "paddingX": { "type": "number" },
        "paddingY": { "type": "number" },
        "fitBoxToContent": { "type": "boolean" }
      }
    },
    "InputBoxConfig": {
      "type": "object",
      "properties": {
        "removeBorder": { "type": "boolean" }
      }
    },
    "MiscConfig": {
      "type": "object",
      "properties": {
        "showVersion": { "type": "boolean" },
        "showPatchesApplied": { "type": "boolean" },
        "expandThinkingBlocks": { "type": "boolean" },
        "enableConversationTitle": { "type": "boolean" },
        "hideStartupBanner": { "type": "boolean" },
        "hideCtrlGToEditPrompt": { "type": "boolean" },
        "hideStartupClawd": { "type": "boolean" },
        "increaseFileReadLimit": { "type": "boolean" }
      }
    },
    "Toolset": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "allowedTools": {
          "oneOf": [
            { "type": "string", "const": "*" },
            { "type": "array", "items": { "type": "string" } }
          ]
        }
      },
      "required": ["name", "allowedTools"]
    }
  }
}
```

### Example

```json
{
  "version": "3.2.2",
  "lastModified": "2025-12-21T10:30:00.000Z",
  "changesApplied": true,
  "ccInstallationPath": "/usr/local/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js",
  "themeId": "dark",
  "themes": [
    {
      "name": "Dark mode",
      "id": "dark",
      "colors": {
        "auto": "rgb(100, 200, 255)",
        "claude": "rgb(255, 150, 100)",
        "error": "rgb(255, 80, 80)",
        "success": "rgb(80, 255, 80)"
      }
    }
  ],
  "thinkingVerbs": {
    "format": "{}… ",
    "verbs": ["Thinking", "Pondering", "Processing"]
  },
  "thinkingStyle": {
    "reverseMirror": true,
    "updateInterval": 120,
    "phases": ["◐", "◓", "◑", "◒"]
  },
  "userMessageDisplay": {
    "format": " > {} ",
    "styling": ["bold"],
    "foregroundColor": "default",
    "backgroundColor": null,
    "borderStyle": "none",
    "borderColor": "rgb(255, 255, 255)",
    "paddingX": 0,
    "paddingY": 0,
    "fitBoxToContent": false
  },
  "inputBox": {
    "removeBorder": false
  },
  "misc": {
    "showVersion": true,
    "showPatchesApplied": true,
    "expandThinkingBlocks": false,
    "enableConversationTitle": true,
    "hideStartupBanner": false,
    "hideCtrlGToEditPrompt": false,
    "hideStartupClawd": false,
    "increaseFileReadLimit": true
  },
  "toolsets": [
    {
      "name": "safe-mode",
      "allowedTools": ["read", "write", "glob"]
    }
  ],
  "selectedToolset": null
}
```

---

## Prompt Data JSON

### Location

```
data/prompts/prompts-{version}.json
~/.tweakcc/prompt-data-cache/prompts-{version}.json
```

### Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "version": {
      "type": "string",
      "description": "Claude Code version"
    },
    "prompts": {
      "type": "array",
      "items": { "$ref": "#/definitions/StringsPrompt" }
    }
  },
  "required": ["version", "prompts"],
  "definitions": {
    "StringsPrompt": {
      "type": "object",
      "properties": {
        "name": {
          "type": "string",
          "description": "Human-readable name"
        },
        "id": {
          "type": "string",
          "description": "Unique identifier (slug)"
        },
        "description": {
          "type": "string",
          "description": "Brief description"
        },
        "pieces": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Text fragments split at variable boundaries"
        },
        "identifiers": {
          "type": "array",
          "items": { "type": "number" },
          "description": "Indices into identifierMap for each boundary"
        },
        "identifierMap": {
          "type": "object",
          "additionalProperties": { "type": "string" },
          "description": "Maps numeric indices to variable names"
        },
        "version": {
          "type": "string",
          "description": "CC version when prompt was last changed"
        }
      },
      "required": ["name", "id", "description", "pieces", "identifiers", "identifierMap", "version"]
    }
  }
}
```

### Example

```json
{
  "version": "2.0.76",
  "prompts": [
    {
      "name": "System Prompt: MCP CLI",
      "id": "system-prompt-mcp-cli",
      "description": "Instructions for using MCP CLI",
      "pieces": [
        "\n\n# MCP CLI Command\n\nUse the ",
        " tool before using the ",
        " tool to understand...",
        ".map((",
        ") => formatServerTool(",
        ", ",
        "))"
      ],
      "identifiers": [0, 1, 2, 3, 4, 3, 4, 4, 5, 6],
      "identifierMap": {
        "0": "READ_TOOL_NAME",
        "1": "WRITE_TOOL_NAME",
        "2": "AVAILABLE_TOOLS_LIST",
        "3": "TOOL_ITEM",
        "4": "FULL_SERVER_TOOL_PATH",
        "5": "FORMAT_SERVER_TOOL_FN",
        "6": "BASH_TOOL_NAME"
      },
      "version": "2.0.55"
    }
  ]
}
```

### Piece/Identifier Reconstruction

```
pieces:      ["Start ", " middle ", " end"]
identifiers: [    0   ,     1     ]

identifierMap: {
  "0": "VAR1",
  "1": "VAR2"
}

Reconstructed:
"Start ${VAR1} middle ${VAR2} end"
```

### Compression Rationale

The pieces/identifiers format provides:
1. **Size reduction**: Variable names stored once
2. **Pattern matching**: Enables regex building for patching
3. **Version tracking**: Each prompt tracks its last change version

---

## System Prompt Markdown

### Location

```
~/.tweakcc/system-prompts/{prompt-id}.md
```

### Format

```markdown
<!--
name: Prompt Name
description: Brief description of the prompt
ccVersion: 2.0.76
variables:
  - VARIABLE_ONE
  - VARIABLE_TWO
  - VARIABLE_THREE
-->

# Prompt Content

Your customized prompt content here.

Use variables like ${VARIABLE_ONE} and ${VARIABLE_TWO}.
```

### Frontmatter Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name |
| `description` | string | Yes | Brief description |
| `ccVersion` | string | Yes | Claude Code version |
| `variables` | string[] | No | Variable placeholders used |

### Variable Syntax

```
${VARIABLE_NAME}
${SETTINGS.preferredName}
${TOOL_LIST}
```

---

## Hash Index Files

### Original Hashes

**Location:** `~/.tweakcc/systemPromptOriginalHashes.json`

**Purpose:** Store baseline hashes from downloaded JSON

```json
{
  "system-prompt-2.0.76": "a1b2c3d4e5f6g7h8i9j0",
  "tool-bash-2.0.76": "k1l2m3n4o5p6q7r8s9t0",
  "agent-explore-2.0.70": "u1v2w3x4y5z6a7b8c9d0"
}
```

### Applied Hashes

**Location:** `~/.tweakcc/systemPromptAppliedHashes.json`

**Purpose:** Store hashes when prompts were last applied

```json
{
  "system-prompt-2.0.76": "a1b2c3d4e5f6g7h8i9j0",
  "tool-bash-2.0.76": "modified123456789abc"
}
```

### Hash Key Format

```
{promptId}-{ccVersion}
```

Example: `system-prompt-2.0.76`

---

## Theme Colors

### Color Properties (62+)

| Category | Properties |
|----------|------------|
| Primary | `auto`, `autoAccept`, `bashBorder`, `claude`, `claudeShimmer` |
| Edit | `edit`, `editShimmer`, `defaultAccent` |
| Status | `error`, `errorShimmer`, `permission`, `permissionShimmer`, `success`, `successShimmer`, `warning`, `warningShimmer` |
| Subagent | `subagentBlue`, `subagentBlueShimmer`, `subagentCyan`, `subagentCyanShimmer`, `subagentGreen`, `subagentGreenShimmer`, `subagentOrange`, `subagentOrangeShimmer`, `subagentPink`, `subagentPinkShimmer`, `subagentPurple`, `subagentPurpleShimmer`, `subagentRed`, `subagentRedShimmer`, `subagentYellow`, `subagentYellowShimmer` |
| Rainbow | `rainbowRed`, `rainbowRedShimmer`, `rainbowOrange`, `rainbowOrangeShimmer`, `rainbowYellow`, `rainbowYellowShimmer`, `rainbowGreen`, `rainbowGreenShimmer`, `rainbowBlue`, `rainbowBlueShimmer`, `rainbowIndigo`, `rainbowIndigoShimmer`, `rainbowViolet`, `rainbowVioletShimmer` |
| Diff | `diffAdded`, `diffAddedWord`, `diffRemoved`, `diffRemovedWord`, `diffAddedBg`, `diffAddedWordBg`, `diffRemovedBg`, `diffRemovedWordBg` |
| Message | `userMessageBg`, `assistantMessageBg`, `systemMessageBg` |

### Color Formats

| Format | Pattern | Example |
|--------|---------|---------|
| RGB | `rgb(r, g, b)` | `rgb(255, 100, 50)` |
| Hex | `#rrggbb` | `#ff6432` |
| Hex Short | `#rgb` | `#f64` |
| HSL | `hsl(h, s%, l%)` | `hsl(20, 100%, 60%)` |
| ANSI | Named colors | `red`, `green`, `blue`, `cyan`, `magenta`, `yellow`, `white`, `black` |

---

## Installation Candidate

### Structure

```typescript
interface InstallationCandidate {
  path: string           // Full path to cli.js or binary
  kind: InstallationKind // 'npm-based' | 'native-binary'
  version: string        // Semantic version (e.g., "2.0.76")
}
```

### Example

```json
[
  {
    "path": "/Users/user/.nvm/versions/node/v20.10.0/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js",
    "kind": "npm-based",
    "version": "2.0.76"
  },
  {
    "path": "/usr/local/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js",
    "kind": "npm-based",
    "version": "2.0.70"
  },
  {
    "path": "/Users/user/.local/bin/claude",
    "kind": "native-binary",
    "version": "2.0.65"
  }
]
```

---

## Backup Files

### cli.js Backup

**Location:** `~/.tweakcc/cli.js.backup`

**Format:** Original unmodified Claude Code JavaScript

### Native Binary Backup

**Location:** `~/.tweakcc/native-binary.backup`

**Format:** Original unmodified Claude Code executable

---

## Conflict HTML Files

### Location

```
~/.tweakcc/system-prompts/{prompt-id}.conflict.html
```

### Structure

```html
<!DOCTYPE html>
<html>
<head>
  <title>Prompt Conflict: {prompt-id}</title>
  <style>
    body { font-family: monospace; margin: 20px; }
    .diff-container { display: flex; gap: 20px; }
    .diff-panel { flex: 1; border: 1px solid #ccc; padding: 10px; }
    .diff-left { background: #fff5f5; }
    .diff-right { background: #f5fff5; }
    .removed { background: #ffcccc; text-decoration: line-through; }
    .added { background: #ccffcc; }
    pre { white-space: pre-wrap; word-wrap: break-word; }
  </style>
</head>
<body>
  <h1>Conflict Detected: {prompt-name}</h1>
  <p>Both you and Anthropic have modified this prompt.</p>

  <div class="diff-container">
    <div class="diff-panel diff-left">
      <h2>Your Version</h2>
      <pre>{user-content}</pre>
    </div>
    <div class="diff-panel diff-right">
      <h2>New Anthropic Version</h2>
      <pre>{new-content}</pre>
    </div>
  </div>

  <h2>Resolution</h2>
  <ol>
    <li>Review both versions above</li>
    <li>Edit ~/.tweakcc/system-prompts/{prompt-id}.md with your merged content</li>
    <li>Delete this .conflict.html file</li>
    <li>Run tweakcc and apply customizations</li>
  </ol>
</body>
</html>
```

---

## .gitignore

### Location

```
~/.tweakcc/.gitignore
```

### Content

```gitignore
# Backups (contain original code)
cli.js.backup
native-binary.backup

# Cached data (can be re-downloaded)
prompt-data-cache/

# Hash indices (regenerated automatically)
systemPromptOriginalHashes.json
systemPromptAppliedHashes.json

# Conflict files (temporary)
*.conflict.html
```

---

## Data Flow Summary

```
GitHub Repository
    │
    ▼ Download
┌──────────────────────────────┐
│ prompts-{version}.json       │  (StringsFile format)
│ - version                    │
│ - prompts[]                  │
│   - pieces[]                 │
│   - identifiers[]            │
│   - identifierMap            │
└──────────────────────────────┘
    │
    ▼ Cache
~/.tweakcc/prompt-data-cache/prompts-{version}.json
    │
    ▼ Sync
┌──────────────────────────────┐
│ system-prompts/{id}.md       │  (Markdown format)
│ - YAML frontmatter           │
│ - Content with ${variables}  │
└──────────────────────────────┘
    │
    ▼ Patch
┌──────────────────────────────┐
│ Claude Code cli.js           │  (Patched JavaScript)
└──────────────────────────────┘
```
