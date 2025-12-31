# tweakcc - Claude Code Customization Tool

> Comprehensive Reference Documentation

## Overview

**tweakcc** is a sophisticated command-line customization tool for Claude Code, the official Claude AI coding assistant. It allows users to customize theme colors, system prompts, thinking verbs, spinner animations, custom toolsets, and various UI elements through intelligent patching of Claude Code's installation files.

| Property | Value |
|----------|-------|
| **Name** | tweakcc |
| **Version** | 3.2.2 |
| **Author** | Piebald LLC (support@piebald.ai) |
| **License** | MIT |
| **Repository** | https://github.com/Piebald-AI/tweakcc |
| **NPM Package** | `tweakcc` |

## Technology Stack

| Component | Technology | Version |
|-----------|------------|---------|
| Language | TypeScript | 5.9.2 |
| UI Framework | React + Ink | 19.1.1 / 6.1.0 |
| Build Tool | Vite (Rolldown) | Latest |
| Test Framework | Vitest | 3.2.4 |
| CLI Parser | Commander | 14.0.0 |
| Terminal Colors | Chalk | 5.5.0 |

## Project Statistics

| Metric | Value |
|--------|-------|
| Total Lines of Code | ~19,612 |
| TypeScript Files | 89 |
| Patch Modules | 27 |
| UI Components | 15 |
| Test Files | 8 |
| Test Lines | 2,600+ |
| Prompt Data Files | 57 |

## Key Features

### 1. Theme Customization
- **7 built-in themes** (dark, light, ANSI-only, colorblind-friendly, monochrome)
- **62+ color properties** per theme
- RGB, HSL, hex, and ANSI color support
- Custom theme creation and editing

### 2. Thinking Indicator Customization
- **100+ thinking verbs** ("Pondering", "Computing", "Razzle-dazzling", etc.)
- Custom spinner animation phases
- Adjustable animation speed (update interval)
- Mirror/reverse animation option
- Format string customization

### 3. System Prompt Management
- Download prompts per Claude Code version from GitHub
- Markdown-based editing with YAML frontmatter
- Variable substitution support (`${VARIABLE_NAME}`)
- Hash-based conflict detection
- HTML diff generation for conflicts

### 4. Tool Restrictions (Toolsets)
- Create named tool groups
- Restrict available tools per session
- Plan mode toolset switching
- Automatic mode-change handling

### 5. User Message Display
- Custom format strings with placeholders
- Text styling (bold, italic, underline, strikethrough, inverse)
- Foreground and background colors
- Border styles (none, single, double, round, bold, etc.)
- Padding configuration (X and Y)

### 6. Miscellaneous Options
- Show/hide tweakcc version indicator
- Show/hide patches applied indication
- Expand thinking blocks by default
- Conversation title management (`/title`, `/rename`)
- Hide startup banner
- Hide Ctrl+G edit prompt hint
- Hide Clawd ASCII art
- Increase file read limit

## Installation

```bash
# Install globally
npm install -g tweakcc

# Or run directly with npx
npx tweakcc

# Or with pnpm
pnpm add -g tweakcc
```

## CLI Usage

```bash
# Interactive mode (full UI)
tweakcc

# Apply saved customizations without UI
tweakcc --apply
tweakcc -a

# Enable debug mode
tweakcc --debug
tweakcc -d

# Combined flags
tweakcc -d -a
```

### Command Options

| Flag | Long Form | Description |
|------|-----------|-------------|
| `-d` | `--debug` | Enable debug mode with verbose output |
| `-a` | `--apply` | Apply saved customizations without interactive UI |
| `-h` | `--help` | Display help information |
| `-V` | `--version` | Display version number |

## Configuration Locations

tweakcc uses a priority-based configuration directory resolution:

| Priority | Location | Condition |
|----------|----------|-----------|
| 1 | `$TWEAKCC_CONFIG_DIR` | Environment variable set |
| 2 | `~/.tweakcc` | Directory exists (backward compat) |
| 3 | `~/.claude/tweakcc` | Claude ecosystem alignment |
| 4 | `$XDG_CONFIG_HOME/tweakcc` | XDG spec compliance |
| 5 | `~/.tweakcc` | Default fallback |

### Configuration Files

```
~/.tweakcc/
├── config.json                      # Main configuration file
├── cli.js.backup                    # Backup of original Claude Code
├── native-binary.backup             # Backup of native binary (if applicable)
├── systemPromptOriginalHashes.json  # Hash index for conflict detection
├── systemPromptAppliedHashes.json   # Applied prompt hashes
├── prompt-data-cache/               # Cached prompt downloads
│   └── prompts-2.0.XX.json          # Cached per-version prompts
├── system-prompts/                  # Editable markdown prompts
│   ├── system-prompt.md
│   ├── tool-bash.md
│   ├── tool-read.md
│   └── ...
└── .gitignore                       # Excludes backups and caches
```

## How It Works

### Patching Approach

tweakcc works by intelligently patching Claude Code's minified JavaScript:

1. **Backup**: Creates a backup of the original `cli.js` or native binary
2. **Restore**: Always starts from the backup to ensure clean state
3. **Patch**: Applies 27 regex-based transformations in sequence
4. **Write**: Writes the modified content back to the installation

### Supported Installation Types

| Type | Description | Detection |
|------|-------------|-----------|
| `npm-based` | Standard npm/yarn/pnpm installation | JavaScript file detection |
| `native-binary` | Standalone executable (Mac/Linux/Windows) | Binary magic number detection |

### Detection Priority

1. `TWEAKCC_CC_INSTALLATION_PATH` environment variable
2. `ccInstallationPath` from config.json
3. `claude` command on PATH (via `which`)
4. 40+ hardcoded search paths across platforms

## Main Menu Options

When running in interactive mode, the main menu provides:

| Option | Description |
|--------|-------------|
| *Apply customizations | Apply all pending changes to Claude Code |
| Themes | Create, edit, and select themes |
| Thinking verbs | Customize the list of thinking action verbs |
| Thinking style | Configure spinner animation and format |
| User message display | Style how user messages appear |
| Misc | Various UI options and toggles |
| Toolsets | Manage tool restriction groups |
| View system prompts | Open system prompts directory |
| Restore original | Restore Claude Code to original state |
| Open config.json | Open configuration in editor |
| Open cli.js | Open Claude Code's source for inspection |
| Exit | Exit tweakcc |

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `TWEAKCC_CONFIG_DIR` | Override configuration directory location |
| `TWEAKCC_CC_INSTALLATION_PATH` | Direct path to Claude Code installation |
| `XDG_CONFIG_HOME` | XDG Base Directory for config |

## Platform Support

| Platform | NPM Install | Native Binary | Node Managers |
|----------|-------------|---------------|---------------|
| macOS | Yes | Yes | nvm, fnm, volta, asdf, mise |
| Linux | Yes | Yes | nvm, fnm, volta, asdf, mise, nodenv |
| Windows | Yes | Yes | nvm4w, fnm, volta |

### Package Manager Support

- npm (global and local)
- yarn (v1 and v2+)
- pnpm
- Bun (with special hard-link handling)
- Homebrew
- MacPorts

### Node Version Manager Support

- nvm (Node Version Manager)
- fnm (Fast Node Manager)
- volta
- asdf
- mise (formerly rtx)
- nodenv
- nvs (Node Version Switcher)
- n

## Documentation Index

| Document | Description |
|----------|-------------|
| [architecture.md](./architecture.md) | System design and data flow |
| [modules.md](./modules.md) | Complete module documentation |
| [patches.md](./patches.md) | All 27 patch implementations |
| [configuration.md](./configuration.md) | Configuration schema and types |
| [system-prompts.md](./system-prompts.md) | System prompt management |
| [installation.md](./installation.md) | Installation detection logic |
| [ui.md](./ui.md) | React/Ink UI components |
| [testing.md](./testing.md) | Test coverage and patterns |
| [data-formats.md](./data-formats.md) | Data structure specifications |

## Version History Highlights

| Version | Date | Changes |
|---------|------|---------|
| v3.2.2 | 2025-12-21 | Increase file read token limit |
| v3.2.0 | 2025-12-14 | Config dir support, `/title` commands, misc view |
| v3.1.0 | 2025-11-15 | Toolset display, `/title` & `/rename` commands |
| v3.0.0 | 2025-11-10 | Toolsets, expanding thinking blocks, misc view |
| v2.0.0 | - | Major refactor with installation detection |
| v1.0.0 | - | Initial release |

## Dependencies

### Production Dependencies

| Package | Purpose |
|---------|---------|
| `react` | UI logic |
| `ink` | Terminal React renderer |
| `commander` | CLI argument parsing |
| `chalk` | Terminal colors |
| `which` | Executable detection |
| `globby` | Glob pattern file matching |
| `gray-matter` | Markdown frontmatter parsing |
| `wasmagic` | WASM-based file type detection |
| `node-lief` | Binary manipulation (optional) |
| `ink-link` | Clickable terminal links |
| `ink-image` | Image rendering in terminal |

### Development Dependencies

| Package | Purpose |
|---------|---------|
| `typescript` | Type checking |
| `vite` | Build tool |
| `vitest` | Test framework |
| `eslint` | Linting |
| `prettier` | Formatting |
| `husky` | Git hooks |

## Quick Start Example

```bash
# 1. Install tweakcc
npm install -g tweakcc

# 2. Run interactive mode
tweakcc

# 3. Select "Themes" from menu
# 4. Choose or create a custom theme
# 5. Adjust colors as needed
# 6. Select "*Apply customizations"
# 7. Launch Claude Code - your customizations are active!

# To restore original:
tweakcc
# Select "Restore original Claude Code"
```

## Troubleshooting

### Common Issues

**Installation not detected:**
- Set `TWEAKCC_CC_INSTALLATION_PATH` environment variable
- Or add `ccInstallationPath` to config.json

**Multiple installations found:**
- Interactive mode shows a picker
- Non-interactive mode requires explicit path

**Bun installation issues:**
- tweakcc handles Bun's hard-linked files automatically
- Uses unlink/write/chmod pattern to break hard links

**Native binary not supported:**
- Ensure `node-lief` can be installed
- Some systems (NixOS) may have library issues
- Falls back gracefully if unavailable

## Contributing

See the [GitHub repository](https://github.com/Piebald-AI/tweakcc) for contribution guidelines.

## License

MIT License - Copyright (c) Piebald LLC
