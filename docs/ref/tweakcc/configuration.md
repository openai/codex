# tweakcc Configuration

> Complete Configuration Schema and Management

## Configuration Directory

### Resolution Priority

tweakcc uses a priority-based configuration directory resolution:

| Priority | Path | Condition |
|----------|------|-----------|
| 1 | `$TWEAKCC_CONFIG_DIR` | Environment variable set and non-empty |
| 2 | `~/.tweakcc` | Directory exists (backward compatibility) |
| 3 | `~/.claude/tweakcc` | Claude ecosystem alignment |
| 4 | `$XDG_CONFIG_HOME/tweakcc` | XDG Base Directory spec |
| 5 | `~/.tweakcc` | Default fallback |

### Directory Structure

```
~/.tweakcc/                          (or resolved config directory)
├── config.json                      # Main configuration file
├── cli.js.backup                    # Backup of original cli.js
├── native-binary.backup             # Backup of native binary (if applicable)
├── systemPromptOriginalHashes.json  # Baseline hashes from downloads
├── systemPromptAppliedHashes.json   # Hashes when last applied
├── .gitignore                       # Excludes backups and caches
├── prompt-data-cache/               # Cached prompt downloads
│   ├── prompts-2.0.70.json
│   ├── prompts-2.0.71.json
│   └── ...
└── system-prompts/                  # Editable markdown prompts
    ├── system-prompt.md
    ├── tool-bash.md
    ├── tool-read.md
    ├── tool-write.md
    ├── agent-explore.md
    └── ...
```

---

## Configuration Schema

### Root Configuration (`TweakccConfig`)

```typescript
interface TweakccConfig {
  // Metadata
  version: string              // tweakcc version that created this config
  lastModified: string         // ISO timestamp of last modification
  changesApplied: boolean      // Whether current changes have been applied

  // Installation
  ccInstallationPath?: string  // Explicit path to Claude Code installation

  // Theme
  themeId: string              // ID of currently selected theme
  themes: Theme[]              // Array of available themes

  // Thinking
  thinkingVerbs: ThinkingVerbsConfig
  thinkingStyle: ThinkingStyleConfig

  // Display
  userMessageDisplay: UserMessageDisplayConfig
  inputBox: InputBoxConfig

  // Features
  misc: MiscConfig

  // Tool Management
  toolsets: Toolset[]
  selectedToolset?: string     // Currently active toolset ID
}
```

---

## Theme Configuration

### Theme Structure

```typescript
interface Theme {
  name: string      // Display name (e.g., "Dark Mode")
  id: string        // Unique identifier (e.g., "dark")
  colors: ThemeColors
}
```

### Theme Colors (62+ Properties)

```typescript
interface ThemeColors {
  // Primary Colors
  auto: string
  autoAccept: string
  bashBorder: string
  claude: string
  claudeShimmer: string
  defaultAccent: string
  edit: string
  editShimmer: string

  // Status Colors
  error: string
  errorShimmer: string
  permission: string
  permissionShimmer: string
  success: string
  successShimmer: string
  warning: string
  warningShimmer: string

  // Subagent Colors (8 base colors)
  subagentBlue: string
  subagentBlueShimmer: string
  subagentCyan: string
  subagentCyanShimmer: string
  subagentGreen: string
  subagentGreenShimmer: string
  subagentOrange: string
  subagentOrangeShimmer: string
  subagentPink: string
  subagentPinkShimmer: string
  subagentPurple: string
  subagentPurpleShimmer: string
  subagentRed: string
  subagentRedShimmer: string
  subagentYellow: string
  subagentYellowShimmer: string

  // Rainbow Colors (7 colors + shimmer variants)
  rainbowRed: string
  rainbowRedShimmer: string
  rainbowOrange: string
  rainbowOrangeShimmer: string
  rainbowYellow: string
  rainbowYellowShimmer: string
  rainbowGreen: string
  rainbowGreenShimmer: string
  rainbowBlue: string
  rainbowBlueShimmer: string
  rainbowIndigo: string
  rainbowIndigoShimmer: string
  rainbowViolet: string
  rainbowVioletShimmer: string

  // Diff Colors
  diffAdded: string
  diffAddedWord: string
  diffRemoved: string
  diffRemovedWord: string
  diffAddedBg: string
  diffAddedWordBg: string
  diffRemovedBg: string
  diffRemovedWordBg: string

  // Message Backgrounds
  userMessageBg: string
  assistantMessageBg: string
  systemMessageBg: string
}
```

### Color Formats

| Format | Example | Notes |
|--------|---------|-------|
| RGB | `rgb(255, 100, 50)` | Most flexible, recommended |
| Hex | `#ff6432` | Standard web format |
| Hex Short | `#f64` | 3-character shorthand |
| HSL | `hsl(20, 100%, 60%)` | Hue-based definition |
| ANSI | `red`, `blue`, `cyan` | 16-color terminal support |

### Built-in Themes

| ID | Name | Description |
|----|------|-------------|
| `dark` | Dark mode | RGB colors for dark terminals |
| `light` | Light mode | RGB colors for light terminals |
| `light-ansi` | Light mode (ANSI only) | 16-color ANSI support |
| `dark-ansi` | Dark mode (ANSI only) | 16-color ANSI support |
| `light-daltonized` | Light (colorblind-friendly) | Daltonized colors |
| `dark-daltonized` | Dark (colorblind-friendly) | Daltonized colors |
| `monochrome` | Monochrome | Accessibility theme |

---

## Thinking Configuration

### Thinking Verbs

```typescript
interface ThinkingVerbsConfig {
  format: string     // Format string with {} placeholder
  verbs: string[]    // List of thinking action verbs
}
```

**Example:**
```json
{
  "format": "{}… ",
  "verbs": [
    "Thinking",
    "Pondering",
    "Computing",
    "Analyzing",
    "Processing"
  ]
}
```

**Default Verbs (100+):**
```
Actualizing, Baking, Computing, Decoding, Elevating,
Formulating, Generating, Hypothesizing, Innovating,
Journeying, Kindling, Leveraging, Mapping, Navigating,
Optimizing, Pondering, Questioning, Razzle-dazzling,
Synthesizing, Thinking, Unpacking, Visualizing, ...
```

### Thinking Style

```typescript
interface ThinkingStyleConfig {
  reverseMirror: boolean    // Reverse animation direction
  updateInterval: number    // Milliseconds between phases
  phases: string[]          // Animation phase characters
}
```

**Example:**
```json
{
  "reverseMirror": true,
  "updateInterval": 120,
  "phases": ["·", "✢", "*", "✦"]
}
```

**Animation Phases Examples:**

| Style | Phases |
|-------|--------|
| Default | `["·", "✢", "*", "✦"]` |
| Dots | `["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]` |
| Circle | `["◐", "◓", "◑", "◒"]` |
| Arrow | `["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"]` |
| Simple | `["-", "\\", "|", "/"]` |

---

## User Message Display

```typescript
interface UserMessageDisplayConfig {
  format: string              // Format with {} placeholder
  styling: TextStyling[]      // Text styling options
  foregroundColor: string | 'default'
  backgroundColor: string | 'default' | null
  borderStyle: BorderStyle
  borderColor: string
  paddingX: number
  paddingY: number
  fitBoxToContent: boolean
}

type TextStyling =
  | 'bold'
  | 'italic'
  | 'underline'
  | 'strikethrough'
  | 'inverse'

type BorderStyle =
  | 'none'
  | 'single'
  | 'double'
  | 'round'
  | 'bold'
  | 'singleDouble'
  | 'doubleSingle'
  | 'classic'
  | 'arrow'
```

**Example:**
```json
{
  "format": " > {} ",
  "styling": ["bold"],
  "foregroundColor": "rgb(0, 255, 0)",
  "backgroundColor": null,
  "borderStyle": "round",
  "borderColor": "rgb(100, 100, 255)",
  "paddingX": 1,
  "paddingY": 0,
  "fitBoxToContent": true
}
```

**Border Styles Visual:**

```
none:         (no border)

single:       ┌─────┐
              │     │
              └─────┘

double:       ╔═════╗
              ║     ║
              ╚═════╝

round:        ╭─────╮
              │     │
              ╰─────╯

bold:         ┏━━━━━┓
              ┃     ┃
              ┗━━━━━┛
```

---

## Input Box Configuration

```typescript
interface InputBoxConfig {
  removeBorder: boolean   // Remove input border
}
```

**Example:**
```json
{
  "removeBorder": false
}
```

---

## Misc Configuration

```typescript
interface MiscConfig {
  showVersion: boolean            // Show tweakcc version in banner
  showPatchesApplied: boolean     // Show patches applied indicator
  expandThinkingBlocks: boolean   // Expand thinking blocks by default
  enableConversationTitle: boolean // Enable /title and /rename commands
  hideStartupBanner: boolean      // Hide Claude Code startup banner
  hideCtrlGToEditPrompt: boolean  // Hide Ctrl+G hint
  hideStartupClawd: boolean       // Hide Clawd ASCII art
  increaseFileReadLimit: boolean  // Increase file read token limit
}
```

**Example:**
```json
{
  "showVersion": true,
  "showPatchesApplied": true,
  "expandThinkingBlocks": false,
  "enableConversationTitle": true,
  "hideStartupBanner": false,
  "hideCtrlGToEditPrompt": false,
  "hideStartupClawd": false,
  "increaseFileReadLimit": true
}
```

---

## Toolsets Configuration

```typescript
interface Toolset {
  name: string                    // Display name
  allowedTools: string[] | '*'    // List of allowed tools or '*' for all
}
```

**Example:**
```json
{
  "toolsets": [
    {
      "name": "safe-mode",
      "allowedTools": ["read", "write", "glob", "grep"]
    },
    {
      "name": "full-access",
      "allowedTools": "*"
    },
    {
      "name": "read-only",
      "allowedTools": ["read", "glob", "grep", "websearch"]
    }
  ],
  "selectedToolset": "safe-mode"
}
```

**Available Tools:**
- `read` - Read files
- `write` - Write files
- `edit` - Edit files
- `bash` - Execute shell commands
- `glob` - File pattern matching
- `grep` - Text search
- `websearch` - Web search
- `webfetch` - Fetch web pages
- `mcp` - MCP server tools
- And more...

---

## Complete Configuration Example

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
    },
    {
      "name": "My Custom Theme",
      "id": "custom-1",
      "colors": {
        "auto": "rgb(255, 100, 200)",
        "claude": "rgb(100, 255, 200)"
      }
    }
  ],
  "thinkingVerbs": {
    "format": "{}… ",
    "verbs": ["Thinking", "Processing", "Analyzing"]
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

## Configuration Management API

### Reading Configuration

```typescript
import { readConfigFile } from './config';

const config = await readConfigFile();
// Returns TweakccConfig with defaults merged
```

**Behavior:**
1. Read `config.json` from config directory
2. Apply migrations for old formats
3. Merge missing properties from defaults
4. Add missing color values to themes
5. Return complete configuration

### Updating Configuration

```typescript
import { updateConfigFile } from './config';

const newConfig = await updateConfigFile((config) => {
  config.themeId = 'custom-1';
  config.thinkingVerbs.verbs.push('Cogitating');
});
// Persists changes and returns updated config
```

**Behavior:**
1. Read current configuration
2. Apply update callback
3. Update `lastModified` timestamp
4. Write to `config.json`
5. Return updated configuration

### Getting Config Directory

```typescript
import { getConfigDir } from './config';

const dir = getConfigDir();
// Returns resolved config directory path
```

### File Path Constants

```typescript
import {
  CONFIG_FILE,
  CLIJS_BACKUP_FILE,
  NATIVE_BINARY_BACKUP_FILE,
  SYSTEM_PROMPTS_DIR,
  PROMPT_CACHE_DIR
} from './config';
```

---

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `TWEAKCC_CONFIG_DIR` | Override config directory | `~/.config/tweakcc` |
| `TWEAKCC_CC_INSTALLATION_PATH` | Direct Claude Code path | `/path/to/cli.js` |
| `XDG_CONFIG_HOME` | XDG base directory | `~/.config` |

---

## Configuration Migrations

### v3.2.0 Migration

**userMessageDisplay restructuring:**

```typescript
// Old format
{
  "userMessageDisplay": {
    "prefix": {
      "format": "$",
      "styling": ["bold"],
      "foregroundColor": "rgb(255,0,0)"
    },
    "message": {
      "format": "{}",
      "styling": ["italic"],
      "foregroundColor": "rgb(0,255,0)"
    }
  }
}

// New format
{
  "userMessageDisplay": {
    "format": "${}",
    "styling": ["bold", "italic"],
    "foregroundColor": "rgb(0,255,0)",
    "backgroundColor": null,
    "borderStyle": "none",
    "borderColor": "rgb(255,255,255)",
    "paddingX": 0,
    "paddingY": 0,
    "fitBoxToContent": false
  }
}
```

### ccInstallationDir Migration

```typescript
// Old format
{
  "ccInstallationDir": "/path/to/claude-code/dist"
}

// New format
{
  "ccInstallationPath": "/path/to/claude-code/dist/cli.js"
}
```

---

## Validation

### Color Validation

```typescript
function isValidColorFormat(color: string): boolean
```

**Valid Formats:**
- `rgb(r, g, b)` where r, g, b are 0-255
- `#rrggbb` hex format
- `#rgb` shorthand hex
- `hsl(h, s%, l%)` HSL format
- Named ANSI colors: `red`, `green`, `blue`, etc.

### Path Validation

Paths are validated during installation detection:
- Must exist on filesystem
- Must be readable
- Must be correct file type (JS or binary)

---

## Backup Strategy

### Automatic Backups

1. On first run, backup original `cli.js` or native binary
2. On version change, create new backup
3. Backups stored in config directory

### Restore Process

```typescript
import { restoreClijsFromBackup } from './installationBackup';

const success = await restoreClijsFromBackup(ccInstInfo);
if (success) {
  // Original restored
  // changesApplied = false
  // Hash indices cleared
}
```

---

## Git Integration

### Generated .gitignore

```gitignore
# tweakcc backups
cli.js.backup
native-binary.backup

# Cached data
prompt-data-cache/

# Hash indices
systemPromptOriginalHashes.json
systemPromptAppliedHashes.json
```

### Safe to Commit

- `config.json` - Your customizations
- `system-prompts/*.md` - Your prompt modifications
