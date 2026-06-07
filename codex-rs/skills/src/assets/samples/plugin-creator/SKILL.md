---
name: plugin-creator
description: Create and scaffold plugin directories for Codex with a required `.codex-plugin/plugin.json`, optional plugin folders/files, valid manifest defaults, and personal-marketplace entries by default. Use when Codex needs to create a new personal plugin, add optional plugin structure, generate or update marketplace entries for plugin ordering and availability metadata, or update an existing local plugin during development with the CLI-driven cachebuster and reinstall flow.
---

# Plugin Creator

## Quick Start

1. Run the scaffold script:

```bash
# Plugin names are normalized to lower-case hyphen-case and must be <= 64 chars.
# The generated folder and plugin.json name are always the same.
# Run from the skill root (the directory containing this `SKILL.md`).
# By default creates in `~/plugins/<plugin-name>`.
python3 scripts/create_basic_plugin.py <plugin-name>
```

2. Edit `<plugin-path>/.codex-plugin/plugin.json` when the request gives specific metadata.
   The scaffold starts with valid defaults and must not contain `[TODO: ...]` placeholders.

3. Generate or update the personal marketplace entry when the plugin should appear in Codex UI ordering:

```bash
# Personal marketplace entries default to `~/.agents/plugins/marketplace.json`.
python3 scripts/create_basic_plugin.py my-plugin --with-marketplace
```

Only specify `--marketplace-name <name>` when the default `personal` marketplace name is already
taken or installed and you need to seed a different new marketplace file:

```bash
python3 scripts/create_basic_plugin.py my-plugin \
  --with-marketplace \
  --marketplace-name team-local
```

Only use a repo/team marketplace when the user specifically asks for that destination:

```bash
python3 scripts/create_basic_plugin.py my-plugin \
  --path <repo-root>/plugins \
  --marketplace-path <repo-root>/.agents/plugins/marketplace.json \
  --with-marketplace
```

When the user specifies a marketplace path, make sure that marketplace is actually installed before
telling the user to reinstall from it. The default personal marketplace file at
`~/.agents/plugins/marketplace.json` is discovered implicitly, but other marketplace paths are not.
On Windows, use the equivalent path under the user profile.

4. Generate/adjust optional companion folders as needed:

```bash
python3 scripts/create_basic_plugin.py my-plugin \
  --path <parent-plugin-directory> \
  --marketplace-path <marketplace-json-path> \
  --with-skills --with-hooks --with-scripts --with-assets --with-mcp --with-apps --with-marketplace
```

`<parent-plugin-directory>` is the directory where the plugin folder `<plugin-name>` will be
created (for example `~/plugins`).

5. Before handing back a generated plugin, run:

```bash
python3 scripts/validate_plugin.py <plugin-path>
```

For updates to an existing local plugin during development, keep the scaffold flow as-is and use the
reference instead of hand-editing marketplace files:

```bash
python3 scripts/update_plugin_cachebuster.py <plugin-path>
```

Prefer the helper default cachebuster unless the user explicitly asks for a specific override.
See `references/installing-and-updating.md` for the expected cachebuster and reinstall flow while iterating on an existing local plugin.

## What this skill creates

- Default marketplace-backed scaffolds use the personal marketplace file at
  `~/.agents/plugins/marketplace.json`, with plugins generally being stored in
  `~/plugins/<plugin-name>/`.
- Creates plugin root at `/<parent-plugin-directory>/<plugin-name>/`.
- Always creates `/<parent-plugin-directory>/<plugin-name>/.codex-plugin/plugin.json`.
- Fills the manifest with the validated schema shape that the ingestion path accepts.
- Creates or updates `~/.agents/plugins/marketplace.json` when `--with-marketplace` is set.
  - If the marketplace file does not exist yet, seed a personal marketplace root before adding the first plugin entry.
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

- Personal-marketplace creation defaults to `~/.agents/plugins/marketplace.json`. Here,
  "personal marketplace" means the marketplace whose file is at that path.
- Repo/team marketplace creation is opt-in through both `--path` and `--marketplace-path`, only
  when the user specifically requests it.
- `--marketplace-name` is an exception path. Use it only when the default `personal` marketplace
  name is already taken and you need to seed a different new marketplace file.
- Do not use `--marketplace-name` to rename an existing marketplace file in place. If the file
  already exists, its top-level `name` must already match.
- If the user specifies a different marketplace path, treat that marketplace as needing explicit installation via `codex plugin marketplace add`.
- Prefer `scripts/read_marketplace_name.py` when you need the marketplace name from any
  `marketplace.json` file. With no argument it reads the default personal marketplace; with an
  explicit path it works for repo/team marketplaces too.
- In either location, the generated source path remains `./plugins/<plugin-name>`.
- Marketplace root metadata supports top-level `name` plus optional `interface.displayName`.
- Treat plugin order in `plugins[]` as render order in Codex. Append new entries unless a user explicitly asks to reorder the list.
- For local plugin entries, presentation metadata comes from the plugin's `.codex-plugin/plugin.json`.
- For Git-source plugin entries, add plugin-entry `displayName`, `description`, and `keywords` when the plugin is not installed yet and Codex cannot read its plugin manifest.
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
- The generated plugin entry shape is:

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

- Use `--force` only when intentionally replacing an existing marketplace entry for the same plugin name.
- If the target marketplace file does not exist yet, create it with top-level `"name"`, an `"interface"` object containing `"displayName"`, and a `plugins` array, then add the new entry.

- For a brand-new marketplace file, the root object should look like:

```json
{
  "name": "personal",
  "interface": {
    "displayName": "Personal"
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

- Outer folder name and `plugin.json` `"name"` are always the same normalized plugin name.
- Do not remove required structure; keep `.codex-plugin/plugin.json` present.
- Do not leave `[TODO: ...]` placeholders in plugin manifests.
- Keep `apps` and `mcpServers` out of `plugin.json` unless their companion files are actually created.
- Omit unsupported plugin manifest fields that validation rejects, including `hooks`.
- If creating files inside an existing plugin path, use `--force` only when overwrite is intentional.
- Preserve any existing marketplace `interface.displayName`.
- When generating marketplace entries, always write `policy.installation`, `policy.authentication`, and `category` even if their values are defaults.
- Add `policy.products` only when the user explicitly asks for that override.
- Keep marketplace `source.path` relative to the selected marketplace root as `./plugins/<plugin-name>`.
- Only use `--marketplace-name` when creating a new marketplace file whose name should not be
  `personal` because that name is already taken or installed elsewhere.
- If Codex would need approval to write the marketplace file, ask for that approval before
  proceeding. If the user prefers to run the write themselves, provide the exact scaffold command
  and then continue from validation or subsequent plugin edits instead of leaving the workflow
  vague.
- For updates to an existing local plugin during development, do not hand-edit marketplace config
  or `marketplace.json`. Use the update flow documented in
  `references/installing-and-updating.md` and `scripts/update_plugin_cachebuster.py`.
- Do not tell the user to run `codex plugin marketplace add` for the default personal-marketplace
  flow. That command is for explicit non-default marketplace configuration, not for the standard
  `~/.agents/plugins/marketplace.json` path.
- If the user provided a non-default `--marketplace-path`, make sure that marketplace is installed
  before giving reinstall instructions. Use `codex plugin marketplace add <path-to-marketplace-root>`
  when that explicit marketplace has not been configured yet.
- When the workflow created or updated a marketplace-backed plugin, end the final user-facing
  response with a short Codex app handoff. Say `To view this in the Codex app:` and write
  `View <normalized plugin name>` and `Share <normalized plugin name>` as Markdown links, not raw
  URLs or code spans.
- The View deeplink uses `codex://plugins/<normalized plugin name>?marketplacePath=<absolute marketplace.json path>`.
  The Share deeplink uses the same URL with `&mode=share`.
- Replace the placeholders with the real normalized plugin name and absolute `marketplace.json`
  path from the scaffolded plugin. URL-encode the path segment and query value when needed.
- Do not add `pluginName` or `hostId` query parameters to these deeplinks. Codex derives both after
  the user clicks the link.
- Do not emit the `View <normalized plugin name>` or `Share <normalized plugin name>` links when no marketplace entry was
  created or updated.

## Reference to exact spec sample

For the exact canonical sample JSON for both plugin manifests and marketplace entries, use:

- `references/plugin-json-spec.md`
- `references/installing-and-updating.md` for update/reinstall guidance while
  iterating on an existing local plugin, plus the new-thread pickup behavior after reinstall

## Validation

After editing `SKILL.md`, run:

```bash
python3 ../skill-creator/scripts/quick_validate.py .
```

Before handing back a generated plugin, run:

```bash
python3 scripts/validate_plugin.py <plugin-path>
```
