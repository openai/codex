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
    "logoDark": "./assets/logo-dark.png",
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
- `mcpServers` (`string` or `object`): MCP config path, or an object whose keys are MCP server names and whose values are MCP server config objects.
- `apps` (`string`): App manifest path for plugin integrations.
- `interface` (`object`): Interface/UX metadata block for plugin presentation.

`mcpServers` may be declared as a companion file path:

```json
{
  "mcpServers": "./.mcp.json"
}
```

Or as an object directly in `plugin.json`:

```json
{
  "mcpServers": {
    "counter": {
      "type": "http",
      "url": "https://sample.example/counter/mcp"
    }
  }
}
```

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
- `logoDark` (`string`): Optional path to the logo asset used in dark mode.
- `screenshots` (`array` of `string`): List of screenshot asset paths.
  - Screenshot entries must be PNG filenames and stored under `./assets/`.
  - Keep file paths relative to plugin root.

### Path conventions and defaults

- Path values should be relative and begin with `./`.
- `skills`, `hooks`, and string-valued `mcpServers` are supplemented on top of default component discovery; they do not replace defaults.
- Custom path values must follow the plugin root convention and naming/namespacing rules.
- This repo’s scaffold writes `.codex-plugin/plugin.json`; treat that as the manifest location this skill generates.

# Scheduled Task Template JSON sample spec

```json
{
  "name": "Morning inbox triage",
  "prompt": "Review my inbox since the previous workday and surface messages that need attention, with a prioritized next-action list.",
  "schedule": {
    "type": "weekdays",
    "time": "08:30"
  }
}
```

## Field guide

### Top-level fields

- `name` (`string`): Nonempty user-facing template name.
- `prompt` (`string`): Nonempty instruction copied into the user's Scheduled task.
- `schedule` (`object`): Default cadence using one of the schedule shapes below.

### `schedule` fields

- `type` (`string`): One of `hourly`, `daily`, `weekdays`, or `weekly`.
- `intervalHours` (`integer`): Positive interval required by `hourly` schedules.
- `days` (`array` of `string`, optional for `hourly`): Nonempty, unique subset of `MO`, `TU`,
  `WE`, `TH`, `FR`, `SA`, and `SU`.
- `time` (`string`): Local wall-clock time in 24-hour `HH:MM` format from `00:00` through `23:59`.

Hourly schedules may optionally restrict the days on which they run:

```json
{
  "type": "hourly",
  "intervalHours": 2,
  "days": ["MO", "TU", "WE", "TH", "FR"]
}
```

Omitting `days` from an hourly schedule runs it every day.

Daily and weekday schedules require a local wall-clock time:

```json
{ "type": "daily", "time": "09:00" }
```

```json
{ "type": "weekdays", "time": "09:00" }
```

Weekly schedules require one or more weekdays and a local wall-clock time:

```json
{
  "type": "weekly",
  "days": ["TU", "TH"],
  "time": "16:45"
}
```

### Path conventions and defaults

- Place each template at `<plugin-root>/scheduled/<template-key>.json`.
- Use a lowercase kebab-case filename stem as the template key.
- Do not add a `scheduled` field to `plugin.json`; supporting Codex clients discover the reserved
  root directory by convention.
- Use strict standard JSON with no comments or unknown fields. Invalid templates are silently
  omitted from the Codex UI.

### Authoring guidance

Choose a task that genuinely repeats, then confirm its cadence, source scope, expected output, and
whether it may make changes. A Scheduled template does not need to use another component from the
same plugin; a plugin may simply bundle a useful collection of automations.

Keep the default prompt concise but useful before personalization. Include the source and time
window, what deserves attention, the expected output, and an explicit read-only boundary when
appropriate. Avoid personal account identifiers, team names, destinations, and project-specific
assumptions that belong in the user's customized instance.

Write real templates rather than placeholders, then run
`scripts/validate_plugin.py <plugin-path>`. When adding templates to an existing installed plugin,
use the cachebuster and reinstall flow so Codex reads the updated materialized copy.

### Instance semantics

Installing a plugin does not activate its templates. The user reviews a template and explicitly
creates an ordinary user-owned Scheduled task from it. Codex does not retain a live relationship to
the plugin or template, so plugin updates affect future creations only. Describe these as Codex
Scheduled task templates; do not promise availability as ChatGPT or cloud automations.

# Marketplace JSON sample spec

`marketplace.json` depends on where the plugin should live. New plugin creation defaults to the
personal marketplace unless the caller explicitly requests a repo-local destination:

- Personal plugin: `~/.agents/plugins/marketplace.json`
- Repo/team plugin: `<repo-root>/.agents/plugins/marketplace.json`

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
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      },
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
  - `path` (`string`): Relative plugin path based on the marketplace root.
    - Personal plugin in `~/.agents/plugins/marketplace.json`: `./plugins/<plugin-name>`
    - Repo/team plugin: `./plugins/<plugin-name>`
  - The same relative path convention is used for both personal and repo/team marketplaces.
    - Example: with `~/.agents/plugins/marketplace.json`, `./plugins/<plugin-name>` resolves to
      `~/plugins/<plugin-name>`.
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
- Default new plugin creation to the personal marketplace.
- Use a repo/team marketplace only when the user specifically requests that destination.
- Only override the marketplace `name` when the default `personal` name is already taken or
  installed and you need to seed a different new marketplace file.
- Choose marketplace location to match the selected destination:
  - Personal plugin: `~/.agents/plugins/marketplace.json`
  - Repo/team plugin: `<repo-root>/.agents/plugins/marketplace.json`

### Plugin validation notes

- The validator mirrors the workspace plugin ingestion schema so generated plugins follow the same
  manifest contract from the start.
- Plugin manifests must include real values for `name`, `version`, `description`,
  `author.name`, and the required `interface` fields.
- `version` must use strict semver.
- `websiteURL`, `privacyPolicyURL`, and `termsOfServiceURL` must be absolute `https://` URLs when
  present.
- `composerIcon`, `logo`, `logoDark`, and `screenshots` must point to real files inside the plugin archive when
  present.
- `apps` should appear in `plugin.json` only when `.app.json` actually exists.
- `mcpServers` may point to `.mcp.json` or contain the MCP server object directly in
  `plugin.json`.
- Validation rejects unsupported manifest fields such as `hooks`, so the scaffold keeps them out of
  generated manifests.
- Run `scripts/validate_plugin.py <plugin-path>` before handing back a generated plugin. It adds one
  intentional preflight check that rejects leftover `[TODO: ...]` placeholders.
