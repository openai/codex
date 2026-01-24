# codex-plugin

A comprehensive plugin system for Codex, providing extensibility through commands, skills, agents, hooks, and more.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Plugin Structure](#plugin-structure)
- [Installation Sources](#installation-sources)
- [Installation Scopes](#installation-scopes)
- [CLI Commands](#cli-commands)
- [Creating Plugins](#creating-plugins)
- [Ecosystem Boundary](#ecosystem-boundary-codex-vs-claude-code)
- [Migration from Claude Code](#migration-from-claude-code-plugins)
- [Programmatic Usage](#programmatic-usage)
- [Important Notes](#important-notes--caveats)

---

## Overview

The codex-plugin crate provides a complete plugin ecosystem for Codex, enabling:

- **7 Component Types**: Commands, Skills, Agents, Hooks, Output Styles, MCP Servers, LSP Servers
- **Multi-Scope Installation**: Managed, User, Project, and Local scopes
- **Multiple Sources**: Install from GitHub, Git, NPM, Pip, or local paths
- **Marketplace System**: Curated plugin collections with versioning
- **Enable/Disable Management**: Per-plugin state with persistence

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         USER INTERFACE                          │
│  /plugin install <source>                                       │
│  /plugin enable/disable/list/update                            │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        COMMAND LAYER                            │
│  CLI Parser (cli.rs) → TUI Handler (slash_command_ext.rs)       │
└──────────────────────────────┬──────────────────────────────────┘
                               │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
        ▼                      ▼                      ▼
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  MARKETPLACE  │      │ INSTALLATION  │      │    STATE      │
│   MANAGER     │      │    ENGINE     │      │   MANAGER     │
│               │      │               │      │               │
│ Add/Remove    │      │ Fetch source  │      │ Enable/Disable│
│ Update/List   │      │ Validate      │      │ Get installed │
│ Find plugin   │      │ Install       │      │ Save registry │
│               │      │               │      │               │
│ marketplace/  │      │ installer/    │      │ registry/     │
│ schema.rs     │      │ mod.rs        │      │ settings.rs   │
└───────┬───────┘      └───────┬───────┘      └───────┬───────┘
        │                      │                      │
        └──────────────────────┼──────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        STORAGE LAYER                            │
│                                                                 │
│  ~/.codex/                                                      │
│  ├── plugins/                                                   │
│  │   ├── .marketplaces.json       ← Marketplace sources         │
│  │   ├── marketplaces/            ← Cached marketplace data     │
│  │   ├── cache/                   ← Installed plugin files      │
│  │   │   └── <marketplace>/<plugin>/<version>/                  │
│  │   └── npm-cache/               ← NPM package cache           │
│  ├── installed_plugins_v2.json    ← V2 registry (with scopes)   │
│  └── settings.json                ← Enable/disable state        │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                       LOADING LAYER                             │
│  PluginLoader → LoadedPlugin → Component Extraction             │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Commands │  │  Skills  │  │  Agents  │  │  Hooks   │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                      │
│  │ MCP Svrs │  │ LSP Svrs │  │  Styles  │                      │
│  └──────────┘  └──────────┘  └──────────┘                      │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        RUNTIME LAYER                            │
│                                                                 │
│  PluginInjector → InjectionReport                               │
│  • Commands → Slash commands (/plugin-name:command)             │
│  • Skills → Available to LLM                                    │
│  • Agents → Available in Task tool                              │
│  • Hooks → Merged with user hooks                               │
│  • MCP Servers → Merged with user MCP config                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## Plugin Structure

A Codex plugin has the following directory structure:

```
my-plugin/
├── .codex-plugin/            # Required: Plugin manifest directory
│   └── plugin.json           # Plugin manifest file
├── commands/                 # Slash commands (markdown files)
│   ├── format.md            # → /my-plugin:format
│   └── lint.md              # → /my-plugin:lint
├── skills/                   # Skills (directories with SKILL.md)
│   └── auto-format/
│       └── SKILL.md
├── agents/                   # Agent definitions (markdown/json)
│   └── code-reviewer.md
├── hooks/                    # Hook configurations
│   └── hooks.json
├── output-styles/            # Output formatting templates
│   └── compact.json
└── mcp-servers/              # MCP server configurations
    └── .mcp.json
```

### Manifest Schema (plugin.json)

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "A sample Codex plugin",
  "author": {
    "name": "Your Name",
    "email": "you@example.com",
    "url": "https://github.com/you"
  },
  "homepage": "https://github.com/you/my-plugin",
  "repository": "https://github.com/you/my-plugin",
  "license": "MIT",
  "keywords": ["formatting", "linting"],

  "commands": "commands/",
  "skills": "skills/",
  "agents": ["agents/code-reviewer.md"],
  "hooks": "hooks/hooks.json",
  "outputStyles": "output-styles/",

  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "my-mcp-server"]
    }
  },

  "lspServers": {
    "rust-analyzer": {
      "command": "rust-analyzer",
      "args": [],
      "languages": ["rust"]
    }
  }
}
```

### Manifest Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Plugin name (kebab-case, no spaces) |
| `version` | string | No | Semantic version (e.g., "1.0.0") |
| `description` | string | No | Brief description |
| `author` | object | No | Author info (name, email, url) |
| `homepage` | string | No | Plugin homepage URL |
| `repository` | string | No | Repository URL |
| `license` | string | No | SPDX license identifier |
| `keywords` | string[] | No | Discovery keywords |
| `commands` | string/string[]/object | No | Command definitions |
| `skills` | string/string[] | No | Skill definitions |
| `agents` | string/string[] | No | Agent definitions |
| `hooks` | string/object/string[] | No | Hook configurations |
| `outputStyles` | string/string[] | No | Output style definitions |
| `mcpServers` | string/object/string[] | No | MCP server configurations |
| `lspServers` | string/object/string[] | No | LSP server configurations |

---

## Installation Sources

Codex supports installing plugins from multiple sources:

| Source | Syntax | Examples |
|--------|--------|----------|
| **Local** | `./path`, `/path`, `local:path` | `/plugin install ./my-plugin` |
| **GitHub** | `owner/repo`, `github:owner/repo@tag` | `/plugin install anthropic/my-plugin`<br>`/plugin install github:user/repo@v1.0.0` |
| **Git** | `https://...git` | `/plugin install https://gitlab.com/user/plugin.git` |
| **NPM** | `npm:package`, `npm:@scope/pkg@version` | `/plugin install npm:my-plugin`<br>`/plugin install npm:@scope/plugin@1.0.0` |
| **Pip** | `pip:package`, `pip:pkg==version` | `/plugin install pip:my-plugin`<br>`/plugin install pip:my-plugin>=2.0` |

### Source Details

**Local**: Copies the plugin directory to the cache.

**GitHub**: Clones from `https://github.com/{owner}/{repo}.git`. Supports branch/tag with `@ref`.

**Git**: Clones any Git URL ending in `.git`. Tracks commit SHA for updates.

**NPM**: Runs `npm install` to fetch the package. Caches in `~/.codex/plugins/npm-cache/`.

**Pip**: Runs `pip install --target --no-deps`. Supports version specifiers (`==`, `>=`, `<=`, `~=`) and custom index URLs.

---

## Installation Scopes

Plugins can be installed at different scopes, controlling visibility and priority:

| Scope | Description | Storage Location |
|-------|-------------|------------------|
| `managed` | Enterprise/policy controlled | Policy directory |
| `user` | User-level (default) | `~/.codex/plugins/cache/` |
| `project` | Project-specific | `<project>/.codex/plugins/` |
| `local` | Development/marketplace | Marketplace directory |

### Resolution Priority

When a plugin is installed at multiple scopes, the highest-priority installation is used:

1. **local** (highest)
2. **project** (if in project context)
3. **user**
4. **managed** (lowest)

---

## CLI Commands

### Plugin Management

```bash
# Install a plugin
/plugin install <source> [--marketplace <name>] [--scope <scope>]

# Examples:
/plugin install github:anthropic/my-plugin
/plugin install npm:@scope/plugin@1.0.0 --scope user
/plugin install ./local/plugin --scope project

# Uninstall a plugin
/plugin uninstall <plugin-id> [--scope <scope>]

# Enable/disable a plugin
/plugin enable <plugin-id>
/plugin disable <plugin-id>

# List installed plugins
/plugin list [--scope <scope>]

# Update a plugin
/plugin update <plugin-id> [--scope <scope>]

# Validate a plugin directory
/plugin validate [<path>]
```

### Marketplace Management

```bash
# Add a marketplace source
/plugin marketplace add <name> <source>

# Examples:
/plugin marketplace add official github:anthropic/codex-plugins
/plugin marketplace add internal https://plugins.company.com/manifest.json

# Remove a marketplace
/plugin marketplace remove <name>

# List marketplaces
/plugin marketplace list

# Update marketplace(s)
/plugin marketplace update          # Update all
/plugin marketplace update <name>   # Update specific
```

### Command Aliases

| Command | Alias |
|---------|-------|
| `install` | `i` |
| `uninstall` | `u`, `rm` |
| `enable` | `on` |
| `disable` | `off` |
| `list` | `ls` |
| `marketplace` | `mp` |

---

## Creating Plugins

### Minimal Plugin

```
my-plugin/
├── .codex-plugin/
│   └── plugin.json
└── commands/
    └── hello.md
```

**plugin.json**:
```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "commands": "commands/"
}
```

**commands/hello.md**:
```markdown
Say hello to the user in a friendly manner.
```

### Command Plugin

Commands are markdown files in the `commands/` directory. The filename (without `.md`) becomes the command name.

**commands/review.md**:
```markdown
---
description: Review the current file for issues
argumentHint: [file]
model: opus
allowedTools:
  - Read
  - Grep
---

Review the specified file for:
1. Code quality issues
2. Potential bugs
3. Performance concerns

Provide actionable suggestions for improvement.
```

Usage: `/my-plugin:review src/main.rs`

### Skill Plugin

Skills are directories containing a `SKILL.md` file with YAML frontmatter:

**skills/auto-format/SKILL.md**:
```markdown
---
name: auto-format
description: Automatically format code files
when_to_use: When the user asks for code formatting or cleanup
allowed-tools:
  - Read
  - Write
  - Edit
---

# Auto Format Skill

When invoked, analyze the code structure and apply consistent formatting:

1. Identify the language from file extension
2. Apply appropriate formatting rules
3. Preserve semantic meaning
4. Report changes made
```

### Agent Plugin

Agents are markdown files defining custom agent behavior:

**agents/code-reviewer.md**:
```markdown
---
name: code-reviewer
description: Thorough code review agent
model: opus
tools:
  - Read
  - Grep
  - Glob
---

You are a thorough code reviewer. When reviewing code:

1. Check for bugs and logic errors
2. Evaluate code style and consistency
3. Look for security vulnerabilities
4. Suggest performance improvements
5. Verify test coverage
```

### Hook Plugin

Hooks respond to events in the Codex lifecycle:

**hooks/hooks.json**:
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "./validate.sh",
            "timeout": 5000,
            "statusMessage": "Validating changes..."
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "script",
            "script": "./post-action.js"
          }
        ]
      }
    ]
  }
}
```

### MCP Server Plugin

**plugin.json** (partial):
```json
{
  "mcpServers": {
    "database": {
      "command": "npx",
      "args": ["-y", "@my-org/db-mcp-server"],
      "env": {
        "DB_HOST": "localhost"
      }
    }
  }
}
```

Or reference an external config:

```json
{
  "mcpServers": ".mcp.json"
}
```

---

## Ecosystem Boundary (Codex vs Claude Code)

Codex plugins use a **separate ecosystem** from Claude Code plugins:

| Aspect | Codex | Claude Code |
|--------|-------|-------------|
| Manifest directory | `.codex-plugin/` | `.claude-plugin/` |
| Home directory | `~/.codex/` | `~/.claude/` |
| Schema | Identical | Identical |

**Plugins are not directly compatible** between the two systems due to different directory names. However, the manifest schema is identical, enabling easy migration.

### Why Separate Ecosystems?

1. **Namespace isolation**: Prevents conflicts between Codex and Claude Code installations
2. **Independent evolution**: Each system can evolve its plugin format independently
3. **Clear ownership**: Makes it obvious which system manages which plugins

---

## Migration from Claude Code Plugins

To use a Claude Code plugin in Codex:

### Manual Migration

1. Copy or clone the plugin
2. Rename the manifest directory:
   ```bash
   mv .claude-plugin .codex-plugin
   ```
3. Install the plugin:
   ```bash
   /plugin install ./path-to-plugin
   ```

### Automated Migration Script

```bash
#!/bin/bash
# convert-to-codex.sh

PLUGIN_DIR="${1:-.}"

if [ -d "$PLUGIN_DIR/.claude-plugin" ]; then
    mv "$PLUGIN_DIR/.claude-plugin" "$PLUGIN_DIR/.codex-plugin"
    echo "Converted: $PLUGIN_DIR"
else
    echo "No .claude-plugin directory found in $PLUGIN_DIR"
    exit 1
fi
```

Usage:
```bash
./convert-to-codex.sh /path/to/claude-plugin
/plugin install /path/to/claude-plugin
```

---

## Programmatic Usage

For Rust developers integrating the plugin system:

### Loading Plugins

```rust
use codex_plugin::{PluginService, PluginLoader, PluginSettings};
use std::path::Path;

async fn load_plugins(codex_home: &Path, project_path: Option<&Path>) {
    // Create the plugin service
    let service = PluginService::new(codex_home).await.unwrap();

    // Load all enabled plugins
    service.load_all(project_path).await.unwrap();

    // Access loaded components
    let skills = service.get_skills().await;
    let agents = service.get_agents().await;
    let hooks = service.get_hooks().await;
    let commands = service.get_commands().await;
    let mcp_servers = service.get_mcp_servers().await;
    let output_styles = service.get_output_styles().await;

    // Check injection report
    if let Some(report) = service.get_injection_report().await {
        println!("Loaded: {} skills, {} agents, {} hooks",
            report.skills_injected,
            report.agents_injected,
            report.hooks_injected
        );
    }
}
```

### Installing Plugins

```rust
use codex_plugin::{
    PluginInstaller, PluginSource,
    registry::{PluginRegistryV2, InstallScope}
};
use std::sync::Arc;

async fn install_plugin(codex_home: &Path) {
    // Create registry and installer
    let registry = Arc::new(PluginRegistryV2::new(codex_home));
    registry.load().await.unwrap();

    let cache_dir = codex_home.join("plugins").join("cache");
    let installer = PluginInstaller::new(registry, cache_dir);

    // Install from different sources
    let source = PluginSource::github("anthropic/my-plugin");
    // or: PluginSource::npm("@scope/my-plugin").with_version("1.0.0")
    // or: PluginSource::pip("my-plugin").with_version("2.0.0")
    // or: PluginSource::local("/path/to/plugin")

    let entry = installer
        .install(&source, "official", InstallScope::User, None)
        .await
        .unwrap();

    println!("Installed version: {:?}", entry.version);
}
```

### Managing Settings

```rust
use codex_plugin::PluginSettings;

async fn manage_settings(codex_home: &Path) {
    let settings = PluginSettings::new(codex_home);
    settings.load().await.unwrap();

    // Check if enabled (default: true)
    let enabled = settings.is_enabled("my-plugin@official").await;

    // Disable a plugin
    settings.disable("my-plugin@official").await;

    // Toggle state
    let new_state = settings.toggle("my-plugin@official").await;

    // Persist changes
    settings.save().await.unwrap();
}
```

---

## Important Notes & Caveats

### Plugin ID Format

Plugin IDs follow the format: `{plugin-name}@{marketplace-name}`

- Example: `code-formatter@official`
- Regex: `/^[a-z0-9][-a-z0-9._]*@[a-z0-9][-a-z0-9._]*$/i`
- Both parts must be valid identifiers (lowercase alphanumeric, hyphens, dots, underscores)

### Version Tracking

- Git-based sources track commit SHA for update detection
- NPM/Pip sources track version from package metadata
- Updates re-fetch from the original source

### Pip Plugin Limitations

- Installed with `--no-deps` (no transitive dependencies)
- Dependencies must be handled separately or bundled
- Best for pure-Python plugins without external dependencies

### NPM Plugin Cache

- Packages cached in `~/.codex/plugins/npm-cache/`
- Each install copies to versioned cache directory
- `node_modules` not included in final installation

### Enable/Disable Behavior

- **New plugins are enabled by default**
- Only disabled plugins are stored in `settings.json`
- Other fields in `settings.json` are preserved during save

### Project Scope Requirements

- Project scope requires a project path context
- Plugins installed to `<project>/.codex/plugins/`
- Only visible when working in that project

### Marketplace Sources

Supported marketplace source types:
- **URL**: HTTP(S) endpoint returning marketplace manifest
- **GitHub**: `owner/repo` shorthand
- **Git**: Any Git URL
- **File**: Local JSON file
- **Directory**: Local directory with `marketplace.json`

### Error Handling

This crate uses `anyhow::Result` for error handling (not `CodexErr`), following the pattern for utility crates in the workspace.

---

## License

See the main Codex repository license.
