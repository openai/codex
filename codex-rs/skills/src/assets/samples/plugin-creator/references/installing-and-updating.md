# Updating Existing Local Plugins

Use this reference when a plugin already exists and the request is about updating the plugin during 
local development.

All scripts here are specified relative to the skill root. Update the path for running the scripts
depending on your current working directory.

## When To Use This Flow

Use this flow when all of the following are true:

- the plugin already exists locally
- the marketplace entry already points at the plugin source you are editing
- the user wants Codex to see the updated plugin without manually editing marketplace files

If the user still needs the initial plugin entry or marketplace structure created, use the scaffold
flow first and only then switch to this reinstall flow.

## Update Loop

1. Update the plugin manifest to a single Codex cachebuster suffix:

```bash
python3 scripts/update_plugin_cachebuster.py \
  <plugin-path>
```

Prefer the default helper behavior here. If you omit `--cachebuster`, the helper uses a UTC
timestamp down to seconds, which is the recommended path for routine local iteration.

Only use a manual cachebuster override when the user explicitly asks for one or when a workflow
outside Codex depends on a specific token:

```bash
python3 scripts/update_plugin_cachebuster.py \
  <plugin-path> \
  --cachebuster local-20260519-184516
```

2. For the default scaffolded flow, read the marketplace name from the personal marketplace file:

```bash
python3 scripts/read_marketplace_name.py
```

Here, "personal marketplace" means the marketplace whose file is at
`~/.agents/plugins/marketplace.json`. On Windows, use the equivalent path under the user profile.
The helper uses Python's home-directory resolution and prints the marketplace name to use when
constructing the install command.

To read the name from a different marketplace file, pass the path directly:

```bash
python3 scripts/read_marketplace_name.py --marketplace-path <path-to-marketplace.json>
```

3. Reinstall from that marketplace name:

```bash
codex plugin add <plugin-name>@<marketplace-name-from-marketplace-json>
```

The default personal marketplace is discovered implicitly from
`~/.agents/plugins/marketplace.json`. You do not need `codex plugin marketplace add` for that
path, and `codex plugin marketplace list` is not the right check for whether that default
marketplace exists.

4. If the plugin is not using the personal marketplace file, check which configured local
   marketplace is actually surfacing that plugin:

```bash
codex plugin list
```

If the plugin is not in the personal marketplace file, confirm which marketplace entry points at
the plugin source you are editing and make sure that marketplace is still local. If it is a
different local marketplace, reinstall from that marketplace name instead of forcing the personal
marketplace flow. If it is not local, stop and help the user resolve the mismatch before
continuing.

5. If the plugin lives in a different confirmed local marketplace, substitute that marketplace
   name:

```bash
codex plugin add <plugin-name>@<local-marketplace>
```

6. Prompt the user to use a new thread to try the updated plugin, so that Codex picks up new skills
   and tools.

## Cachebuster Policy

- Preserve the existing version prefix and replace only the suffix.
- Treat the preserved prefix as everything before `+`.
- Use the format:

```text
<base-version>+codex.<cachebuster>
```

Examples:

- `0.1.0` → `0.1.0+codex.local-20260519-184516`
- `0.1.0+codex.old-token` → `0.1.0+codex.local-20260519-184516`
- `1.2.3-beta.1+codex.prev` → `1.2.3-beta.1+codex.local-20260519-184516`
- `dev-build+other-tag` → `dev-build+codex.local-20260519-184516`

Replace the existing Codex cachebuster instead of appending another one. Do not keep incrementing
numeric version components just to trigger reinstall behavior.

## Marketplace Rules

- Marketplace manipulation should happen through commands, not by hand-editing `marketplace.json`
  or `config.toml` during this update/reinstall flow.
- Prefer the personal marketplace file for the default scaffolded flow.
- Read the personal marketplace name with
  `python3 scripts/read_marketplace_name.py` and use the printed value when constructing
  `codex plugin add <plugin-name>@<marketplace-name>`.
- For non-default marketplace files, use
  `python3 scripts/read_marketplace_name.py --marketplace-path <path-to-marketplace.json>` to read
  the name before constructing reinstall commands.
- Do not tell the user to run `codex plugin marketplace add` for the default personal-marketplace
  flow. That marketplace is discovered implicitly by Codex.
- If the user specified a different marketplace path, make sure that marketplace is installed
  before giving install or reinstall instructions. Non-default marketplace paths are not
  discovered implicitly.
- Use `codex plugin list` when the plugin lives in a different configured marketplace and you need
  to confirm which marketplace is surfacing that plugin.
- If a non-default local marketplace has not been configured yet, install it with
  `codex plugin marketplace add <path-to-marketplace-root>` before telling the user to run
  `codex plugin add <plugin-name>@<marketplace-name>`.
- If the plugin is not in the personal marketplace file, confirm that the selected marketplace is
  local before telling the user to reinstall from it.
- If the selected marketplace is not local, stop and help the user resolve that mismatch rather
  than pretending the normal local reinstall flow applies.
- If the plugin source is not already the source referenced by the chosen marketplace entry, stop
  and fix that first. This update flow does not rewrite marketplace entries.

## After Reinstall

After reinstalling, prompt the user to start a new thread for testing. That is the safe boundary for
picking up the updated plugin and its MCP tools.
