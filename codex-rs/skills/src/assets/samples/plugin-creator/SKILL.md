---
name: plugin-creator
description: Create and scaffold plugin directories for Codex with a required `.codex-plugin/plugin.json`, optional plugin folders/files, and baseline placeholders you can edit before publishing or testing. Use when Codex needs to create a plugin in the current repo or as a local plugin, add optional plugin structure, or generate or update marketplace entries for plugin ordering and availability metadata.
---

# Plugin Creator

## Quick Start

Before running the scaffold script, ask whether the plugin should live in the current repo or as a local plugin.

- If the user does not specify a location, default to the current repo and create the plugin under `<cwd>/plugins`.
- For a local plugin, use `${CODEX_HOME:-$HOME/.codex}/plugins` as the plugin parent and `$HOME/.agents/plugins/marketplace.json` as the marketplace path.

1. Run the scaffold script for the current repo default:

```bash
# Plugin names are normalized to lower-case hyphen-case and must be <= 64 chars.
# The generated folder and plugin.json name are always the same.
# Run from repo root (or replace .agents/... with the absolute path to this SKILL).
# If the user did not choose a location, default to ./plugins/<plugin-name>.
python3 .agents/skills/plugin-creator/scripts/create_basic_plugin.py <plugin-name>
```

2. Open `<plugin-path>/.codex-plugin/plugin.json` and replace `[TODO: ...]` placeholders.

3. Generate or update the current repo marketplace entry when the plugin should appear in Codex UI ordering:

```bash
# marketplace.json defaults to <cwd>/.agents/plugins/marketplace.json
python3 .agents/skills/plugin-creator/scripts/create_basic_plugin.py my-plugin --with-marketplace
```

4. For a local plugin instead of a repo plugin, pass explicit paths:

```bash
python3 /absolute/path/to/plugin-creator/scripts/create_basic_plugin.py my-plugin \
  --path "${CODEX_HOME:-$HOME/.codex}/plugins" \
  --with-marketplace \
  --marketplace-path "$HOME/.agents/plugins/marketplace.json"
```

5. Generate/adjust optional companion folders as needed:

```bash
python3 .agents/skills/plugin-creator/scripts/create_basic_plugin.py my-plugin --path <parent-plugin-directory> \
  --with-skills --with-hooks --with-scripts --with-assets --with-mcp --with-apps \
  [--with-marketplace --marketplace-path <marketplace-path>]
```

`<parent-plugin-directory>` is the directory where the plugin folder `<plugin-name>` will be created (for example `~/code/plugins`).

## What this skill creates

- Creates plugin root at `/<parent-plugin-directory>/<plugin-name>/`.
- Always creates `/<parent-plugin-directory>/<plugin-name>/.codex-plugin/plugin.json`.
- Fills the manifest with the full schema shape, placeholder values, and the complete `interface` section.
- When `--with-marketplace` is set, creates or updates the marketplace file for the chosen destination:
  - repo plugin: `<cwd>/.agents/plugins/marketplace.json`
  - local plugin: `~/.agents/plugins/marketplace.json`
  - If the marketplace file does not exist yet, seed top-level `name` plus `interface.displayName` placeholders before adding the first plugin entry.
- `<plugin-name>` is normalized using skill-creator naming rules:
  - `My Plugin` → `my-plugin`
  - `My--Plugin` → `my-plugin`
  - underscores, spaces, and punctuation are converted to `-`
  - result is lower-case hyphen-delimited with consecutive hyphens collapsed
- Supports optional creation of:
  - `skills/`
  - `hooks/`
  - `scripts/`
  - `assets/`
  - `.mcp.json`
  - `.app.json`

## Marketplace workflow

- Ask whether the user wants a repo plugin or a local plugin before choosing paths.
- If the user does not specify, default to the current repo:
  - plugin parent: `<cwd>/plugins`
  - marketplace: `<cwd>/.agents/plugins/marketplace.json`
- For a local plugin, use:
  - plugin parent: `${CODEX_HOME:-$HOME/.codex}/plugins`
  - marketplace: `~/.agents/plugins/marketplace.json`
- Marketplace root metadata supports top-level `name` plus optional `interface.displayName`.
- Treat plugin order in `plugins[]` as render order in Codex. Append new entries unless a user explicitly asks to reorder the list.
- `displayName` belongs inside the marketplace `interface` object, not individual `plugins[]` entries.
- Each generated marketplace entry must include all of:
  - `policy.installation`
  - `policy.authentication`
  - `category`
- Default new entries to:
  - `policy.installation: "AVAILABLE"`
  - `policy.authentication: "ON_INSTALL"`
- Override defaults only when the user explicitly specifies another allowed value.
- Allowed `policy.installation` values:
  - `NOT_AVAILABLE`
  - `AVAILABLE`
  - `INSTALLED_BY_DEFAULT`
- Allowed `policy.authentication` values:
  - `ON_INSTALL`
  - `ON_USE`
- Treat `policy.products` as an override. Omit it unless the user explicitly requests product gating.
- The generated repo plugin entry shape is:

```json
{
  "name": "plugin-name",
  "source": {
    "source": "local",
    "path": "./plugins/plugin-name"
  },
  "policy": {
    "installation": "AVAILABLE",
    "authentication": "ON_INSTALL"
  },
  "category": "Productivity"
}
```

- For a local plugin in `~/.agents/plugins/marketplace.json`, use:

```json
{
  "name": "plugin-name",
  "source": {
    "source": "local",
    "path": "./.codex/plugins/plugin-name"
  },
  "policy": {
    "installation": "AVAILABLE",
    "authentication": "ON_INSTALL"
  },
  "category": "Productivity"
}
```

- Use `--force` only when intentionally replacing an existing marketplace entry for the same plugin name.
- If the chosen marketplace file does not exist yet, create it with top-level `"name"`, an `"interface"` object containing `"displayName"`, and a `plugins` array, then add the new entry.

- For a brand-new marketplace file, the root object should look like:

```json
{
  "name": "[TODO: marketplace-name]",
  "interface": {
    "displayName": "[TODO: Marketplace Display Name]"
  },
  "plugins": [
    {
      "name": "plugin-name",
      "source": {
        "source": "local",
        "path": "./plugins/plugin-name"
      },
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      },
      "category": "Productivity"
    }
  ]
}
```

## Required behavior

- Ask whether the plugin should live in the current repo or as a local plugin before scaffolding.
- If the user does not specify a location, default to the current repo rooted at `<cwd>`.
- Outer folder name and `plugin.json` `"name"` are always the same normalized plugin name.
- Do not remove required structure; keep `.codex-plugin/plugin.json` present.
- Keep manifest values as placeholders until a human or follow-up step explicitly fills them.
- If creating files inside an existing plugin path, use `--force` only when overwrite is intentional.
- Preserve any existing marketplace `interface.displayName`.
- When generating marketplace entries, always write `policy.installation`, `policy.authentication`, and `category` even if their values are defaults.
- Add `policy.products` only when the user explicitly asks for that override.
- Keep marketplace `source.path` relative to the chosen marketplace root:
  - repo plugin: `./plugins/<plugin-name>`
  - local plugin: `./.codex/plugins/<plugin-name>`

## Reference to exact spec sample

For the exact canonical sample JSON for both plugin manifests and marketplace entries, use:

- `references/plugin-json-spec.md`

## Validation

After editing `SKILL.md`, run:

```bash
python3 <path-to-skill-creator>/scripts/quick_validate.py .agents/skills/plugin-creator
```
