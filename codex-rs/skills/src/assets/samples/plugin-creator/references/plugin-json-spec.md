# Plugin JSON sample spec

```json
{
  "name": "plugin-name",
  "version": "1.2.0",
  "description": "Brief plugin description",
  "author": {
    "name": "Author Name",
    "email": "author@example.com",
    "url": "https://github.com/author"
  },
  "homepage": "https://docs.example.com/plugin",
  "repository": "https://github.com/author/plugin",
  "license": "MIT",
  "keywords": ["keyword1", "keyword2"],
  "skills": "./skills/",
  "hooks": "./hooks.json",
  "mcpServers": "./.mcp.json",
  "apps": "./.app.json",
  "interface": {
    "displayName": "Plugin Display Name",
    "shortDescription": "Short description for subtitle",
    "longDescription": "Long description for details page",
    "developerName": "OpenAI",
    "category": "Productivity",
    "capabilities": ["Interactive", "Write"],
    "websiteURL": "https://openai.com/",
    "privacyPolicyURL": "https://openai.com/policies/row-privacy-policy/",
    "termsOfServiceURL": "https://openai.com/policies/row-terms-of-use/",
    "defaultPrompt": [
      "Summarize my inbox and draft replies for me.",
      "Find open bugs and turn them into Linear tickets.",
      "Review today's meetings and flag scheduling gaps."
    ],
    "brandColor": "#3B82F6",
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": [
      "./assets/screenshot1.png",
      "./assets/screenshot2.png",
      "./assets/screenshot3.png"
    ]
  }
}
```

## Field guide

### Top-level fields

- `name` (`string`): Plugin identifier (kebab-case, no spaces). Required if `plugin.json` is provided and used as manifest name and component namespace.
- `version` (`string`): Plugin semantic version.
- `description` (`string`): Short purpose summary.
- `author` (`object`): Publisher identity.
  - `name` (`string`): Author or team name.
  - `email` (`string`): Contact email.
  - `url` (`string`): Author/team homepage or profile URL.
- `homepage` (`string`): Documentation URL for plugin usage.
- `repository` (`string`): Source code URL.
- `license` (`string`): License identifier (for example `MIT`, `Apache-2.0`).
- `keywords` (`array` of `string`): Search/discovery tags.
- `skills` (`string`): Relative path to skill directories/files.
- `hooks` (`string`): Hook config path.
- `mcpServers` (`string`): MCP config path.
- `apps` (`string`): App manifest path for plugin integrations.
- `interface` (`object`): Interface/UX metadata block for plugin presentation.

### `interface` fields

- `displayName` (`string`): User-facing title shown for the plugin.
- `shortDescription` (`string`): Brief subtitle used in compact views.
- `longDescription` (`string`): Longer description used on details screens.
- `developerName` (`string`): Human-readable publisher name.
- `category` (`string`): Plugin category bucket.
- `capabilities` (`array` of `string`): Capability list from implementation.
- `websiteURL` (`string`): Public website for the plugin.
- `privacyPolicyURL` (`string`): Privacy policy URL.
- `termsOfServiceURL` (`string`): Terms of service URL.
- `defaultPrompt` (`array` of `string`): Starter prompts shown in composer/UX context.
  - Include at most 3 strings. Entries after the first 3 are ignored and will not be included.
  - Each string is capped at 128 characters. Longer entries are truncated.
  - Prefer short starter prompts around 50 characters so they scan well in the UI.
- `brandColor` (`string`): Theme color for the plugin card.
- `composerIcon` (`string`): Path to icon asset.
- `logo` (`string`): Path to logo asset.
- `screenshots` (`array` of `string`): List of screenshot asset paths.
  - Screenshot entries must be PNG filenames and stored under `./assets/`.
  - Keep file paths relative to plugin root.

### Path conventions and defaults

- Path values should be relative and begin with `./`.
- `skills`, `hooks`, and `mcpServers` are supplemented on top of default component discovery; they do not replace defaults.
- Custom path values must follow the plugin root convention and naming/namespacing rules.
- This repo’s scaffold writes `.codex-plugin/plugin.json`; treat that as the manifest location this skill generates.

# Marketplace JSON sample spec

`marketplace.json` always lives at `<repo-root>/.agents/plugins/marketplace.json`.

```json
{
  "name": "openai-curated",
  "interface": {
    "displayName": "ChatGPT Official"
  },
  "plugins": [
    {
      "name": "linear",
      "source": {
        "source": "local",
        "path": "./plugins/linear"
      },
      "installPolicy": "AVAILABLE",
      "authPolicy": "ON_INSTALL",
      "category": "Productivity"
    }
  ]
}
```

## Marketplace field guide

### Top-level fields

- `name` (`string`): Marketplace identifier or catalog name.
- `interface` (`object`, optional): Marketplace presentation metadata.
- `plugins` (`array`): Ordered plugin entries. This order determines how Codex renders plugins.

### `interface` fields

- `displayName` (`string`, optional): User-facing marketplace title.

### Plugin entry fields

- `name` (`string`): Plugin identifier. Match the plugin folder name and `plugin.json` `name`.
- `source` (`object`): Plugin source descriptor.
  - `source` (`string`): Use `local` for this repo workflow.
  - `path` (`string`): Relative plugin path, always `./plugins/<plugin-name>`.
- `policy` (`object`): Marketplace policy block. Always include it.
  - `installation` (`string`): Availability policy.
    - Allowed values: `NOT_AVAILABLE`, `AVAILABLE`, `INSTALLED_BY_DEFAULT`
    - Default for new entries: `AVAILABLE`
  - `authentication` (`string`): Authentication timing policy.
    - Allowed values: `ON_INSTALL`, `ON_USE`
    - Default for new entries: `ON_INSTALL`
  - `products` (`array` of `string`, optional): Product override for this plugin entry. Omit it unless product gating is explicitly requested.
- `category` (`string`): Display category bucket. Always include it.

### Marketplace generation rules

- `displayName` belongs under the top-level `interface` object, not individual plugin entries.
- When creating a new marketplace file from scratch, seed `interface.displayName` alongside top-level `name`.
- Always include `policy.installation`, `policy.authentication`, and `category` on every generated or updated plugin entry.
- Treat `policy.products` as an override and omit it unless explicitly requested.
- Append new entries unless the user explicitly requests reordering.
- Replace an existing entry for the same plugin only when overwrite is intentional.
