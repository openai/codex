# Codex self-knowledge

Use this manual-first route only for genuinely broad Codex setup, orientation, customization, troubleshooting, local-state guidance, or system-map synthesis across skills, plugins, MCP, hooks, `AGENTS.md`, automations, and product surfaces. Mixed Chat/Work/Codex comparisons belong to `official-docs.md` instead.

Narrow Codex documentation questions require official documentation search first, then an actual page open or fetch using an available documentation or official-domain web capability. This includes a single feature such as Codex Goals, a specific setting, documented behavior, exact error, or requested page citation. Search and fetch the exact official topic before inspecting local files or bundled references. Do not fetch the manual, read bundled references, inspect local configuration or caches, or turn a targeted documentation lookup into broad product synthesis. Current or latest model questions follow the model-selection route.

## Start with the manual

Reuse a manual path and outline path already established in the same thread when both remain usable and current. Refresh before relying on a path fetched more than about a day ago, obtained from another thread or uncertain source, or missing likely-current information.

Otherwise, run the bundled manual helper first. Skip it without probing only when policy explicitly makes the session read-only, shell execution unavailable, or every allowed temporary cache location unavailable. Workspace-only write access is not enough: the helper needs an allowed writable temp cache. A guessed sandbox restriction is not evidence that the helper is unavailable.

Resolve `<skill-dir>` to the actual installed skill directory, then run:

```bash
node <skill-dir>/scripts/fetch-codex-manual.mjs
```

The helper automatically chooses the first usable cache location in this order:

1. `$TMPDIR/openai-docs-cache`
2. `%TEMP%\openai-docs-cache`
3. `%TMP%\openai-docs-cache`
4. `/private/tmp/openai-docs-cache`
5. `/tmp/openai-docs-cache`

Use an explicit override only when the allowed cache must be selected manually:

```bash
node <skill-dir>/scripts/fetch-codex-manual.mjs --cache-dir <cache-dir>
```

On Windows, `%TEMP%` and `%TMP%` are discovered automatically; `$env:TEMP\openai-docs-cache` is a typical PowerShell override. The helper handles configured HTTP(S) proxies and falls back to `curl` when needed. Do not require a POSIX-only environment prefix or an unnecessary cache override.

The helper verifies the current source and returns a manual path, outline path, freshness status, and heading outline. Use that outline to locate relevant headings and line ranges, then read or search only the returned manual and outline paths. Do not inspect unrelated repositories, caches, source trees, or local state to establish a public Codex product claim.

For follow-up questions in the same thread, reuse those fresh paths instead of fetching again. If asked whether the manual is current enough to rely on now, rerun the helper when an allowed temp cache is available and answer from its reported status and returned paths.

## Fill only genuine documentation gaps

If the manual answers a claim, stop retrieving sources for that claim. Its official source pages and known anchors are sufficient citation support. Continue the user's broader task when the documentation lookup was only one dependency.

If the helper was legitimately skipped, actually fails, or the fresh manual lacks a material or likely-current claim, use the narrowest official follow-up. Search the exact topic using an available documentation or approved-domain web capability, then actually open or fetch a clearly relevant official result. A page-specific citation can justify the same narrow follow-up.

For an undocumented Codex term, mode, acronym, or exact error, first check adjacent manual terminology. Map it to the closest documented concept when possible. If the exact term is material or likely current, perform one targeted official search-and-fetch; if it remains undocumented, say so. Do not expand into internal knowledge bases, private source trees, guessed roadmap details, or account-specific workarounds.

If official documentation conflicts with a callable capability verified in the current session, explicitly state the conflict and prefer that verified behavior for this environment. Otherwise, resolve unsupported claims with bounded uncertainty or route the user to support, an administrator, or product feedback.

## Choose the smallest matching Codex surface

- Prompt or thread context: one-off task constraints.
- Repository `AGENTS.md`: durable team conventions, commands, and verification expectations; nested files apply more specifically within their subtree.
- Project `.codex/config.toml`: settings for a trusted repository, including sandbox, MCP, hooks, model, and reasoning defaults.
- Global config or global guidance: personal defaults across repositories.
- Skill: a reusable workflow, optionally with focused references or scripts.
- Plugin: an installable bundle of skills, tools, commands, MCP configuration, hooks, apps, assets, or related metadata.
- MCP server or app connector: authorized live external data and actions. Use an authenticated connector, not web search or memory, for private Google Docs, Calendar, Slack, GitHub, Notion, or similar workspace data.
- Automation: scheduled checks, reminders, monitors, or follow-ups; use an existing-thread heartbeat when continuity matters.
- Hook: mechanical enforcement around lifecycle events, tool calls, commands, or edits.

Split requests that combine one-off, durable, repository-scoped, and recurring behavior instead of forcing them onto a single surface. For example, "always do this, but only for this PR" belongs in the current prompt or thread unless the user explicitly wants persistence or enforcement.

For a surface recommendation, state what to use, why it fits, what to avoid, and the manual or official documentation supporting the answer.

For product surfaces, distinguish terminal-first CLI work, editor-attached IDE work, desktop planning or review, hosted cloud execution, in-app browser testing, the user's existing Chrome session, and desktop Computer Use. Keep `config.toml` defaults, `requirements.toml` constraints, and managed or administrator policy separate. An API key does not establish ChatGPT, Codex cloud, connector, or account access.

For plugin or app failures, check the installed bundle, enabled state, connector authorization, MCP setup, restart or new-thread expectations, and workspace policy before inferring a cause. Route billing, entitlements, undocumented rollout labels, and unsupported access paths to the appropriate support or administrative owner.

Memory can provide user preferences or context, but explicit prompt instructions win and memory is not a source for current external facts. Sandbox or network denials require narrowly scoped escalation with a clear justification; destructive commands, writes outside the workspace, and broad access changes require explicit approval.

When a page-specific citation helps, useful official anchors include `concepts/customization#agents-guidance`, `concepts/customization#skills`, `plugins/build#plugin-structure`, `concepts/customization#mcp`, `config-advanced#hooks`, `app/automations#thread-automations`, and `config-reference#configtoml`.
