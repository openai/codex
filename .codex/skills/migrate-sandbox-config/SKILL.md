---
name: migrate-sandbox-config
description: Migrate the current Codex sandbox configuration in config.toml from legacy sandbox_mode and [sandbox_workspace_write] settings to the new permissions structure using default_permissions and [permissions.default]. Use when a user wants to convert their active sandbox config, repair a partial sandbox migration conservatively, or audit whether the current config can be migrated without changing behavior.
---

# Migrate Sandbox Config to Permissions

## Overview

Migrate only the top-level legacy sandbox settings in the target `config.toml`. Rewrite them to `default_permissions = "default"` plus populated `[permissions.default.filesystem]` and optional `[permissions.default.network]`. Prefer skipping with an explanation over changing behavior.

Default to a dry run first. Show the exact proposed config changes and wait for explicit user confirmation before writing the file.

## Workflow

1. Read the target `config.toml` as text.
2. Inspect only these keys before deciding what to do:
   - top-level `sandbox_mode`
   - top-level `[sandbox_workspace_write]`
   - `default_permissions`
   - `[permissions]`
   - any `profiles.<name>.sandbox_mode`
3. Decide whether migration is safe:
   - If top-level `sandbox_mode` is missing, stop. Do not infer one.
   - If active `default_permissions` and a non-empty `[permissions]` table already exist, stop.
   - If `sandbox_mode = "danger-full-access"`, stop.
   - If running on Windows and `sandbox_mode = "workspace-write"`, stop. Current behavior can depend on Windows sandbox support.
4. Build `[permissions.default]` from the live legacy settings:
   - `read-only` -> `[permissions.default.filesystem]` with `":root" = "read"`
   - `workspace-write` -> `":root" = "read"` and `[permissions.default.filesystem.":project_roots"] "." = "write"`
   - Copy each explicit `writable_root` as `"absolute/path" = "write"`
   - If `network_access = true`, add `[permissions.default.network] enabled = true`
   - Preserve `"/tmp" = "write"` only if it was explicitly writable already; do not invent extra temp paths
   - Never migrate internal runtime-only writable roots such as Codex memories directories
5. Preview the proposed migration before touching the file:
   - Show the exact keys that will be added, removed, or replaced
   - Call out behavior-changing fields explicitly, especially:
     - network becoming enabled
     - new writable roots outside the project root
     - `"/tmp"` or `":tmpdir"` write access
   - Mention any remaining profile-scoped legacy sandbox settings that will be left alone
   - Wait for explicit user confirmation before editing the file
6. Apply edits surgically after confirmation:
   - Set `default_permissions = "default"`
   - Remove top-level `sandbox_mode`
   - Remove top-level `[sandbox_workspace_write]`
   - Replace `permissions.default` before writing fresh leaves so stale keys do not survive
   - Avoid creating empty `[permissions]` or `[permissions.default]` parent tables
   - Preserve comments and unrelated settings when possible
7. Verify:
   - Re-read the file
   - Confirm only populated `[permissions.default.*]` leaves were added
   - Confirm `profiles.<name>.sandbox_mode` entries still exist and call them out in the summary

## Partial Configs

If `default_permissions` or `[permissions]` exists but there is still a live top-level legacy `sandbox_mode`, ignore stale `permissions.default` contents and rebuild from the legacy settings.

If `default_permissions` or `[permissions]` exists and there is no top-level legacy `sandbox_mode`, stop and explain that the file is already partially migrated and Codex should not guess the intended behavior.

## Output

Summarize:
- what was migrated
- what was skipped
- any remaining profile-scoped legacy sandbox settings
