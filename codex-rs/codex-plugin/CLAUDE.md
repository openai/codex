# codex-plugin

**Codex-specific** plugin system using **V2 schema only**.

**Important:** This is a Codex-specific plugin ecosystem (not Claude Code compatible).
Plugins use `.codex-plugin/` directory (not `.claude-plugin/`).

## Important Note

**This crate does NOT follow the `*_ext.rs` extension pattern.** Direct modifications to existing files are allowed and preferred for this directory.

## Overview

This crate provides plugin discovery, installation, loading, and injection for Codex.
It uses the V2 schema which supports multi-scope installations.

## Plugin Directory Structure

```
my-plugin/
├── .codex-plugin/        # Codex standard (required)
│   └── plugin.json       # Plugin manifest
├── commands/             # Slash commands
├── skills/               # Skills
├── agents/               # Agent definitions
├── hooks/                # Hooks config
├── output-styles/        # Output templates
└── mcp-servers/          # MCP server configs
```

## Error Handling

This crate uses `anyhow::Result` for error handling (not `CodexErr`), following the
pattern for utility crates in this workspace.

## Key Types

- `InstallScope` - Four scopes: managed, user, project, local
- `InstallEntryV2` - Single installation entry with scope
- `InstalledPluginsV2` - V2 registry format (array of entries per plugin)
- `PluginManifest` - Claude Code-compatible manifest
- `PluginRegistryV2` - Registry manager with scope-aware operations
- `PluginInstaller` - Install/uninstall/update operations
- `PluginLoader` - Load and extract plugin components

## Plugin ID Format

```
{plugin-name}@{marketplace-name}
```

Example: `code-formatter@official`

Regex: `/^[a-z0-9][-a-z0-9._]*@[a-z0-9][-a-z0-9._]*$/i`

## Storage

```
~/.codex/
├── plugins/
│   ├── .marketplaces.json
│   └── cache/<marketplace>/<plugin>/<version>/
├── installed_plugins_v2.json
└── settings.json (enabledPlugins)
```

## Scope Resolution Priority

When resolving which plugin installation to use:
1. `project` (if in project context)
2. `user`
3. `managed`
