# Model migration and prompting

Use this route for model upgrades, migration planning, model-specific prompting, or latest/current/default prompting guidance. First search current official OpenAI documentation for the exact requested topic and model, then open or fetch the relevant official page using an available documentation or official-domain web capability. Do not run a resolver, open bundled references, or rely on a guide URL before completing that official search and actual page fetch.

## Choose the target before loading more context

- **Explicit model target:** Preserve the user's exact requested target, including an explicitly requested GPT-4.1 or GPT-5.4 migration. Do not run the latest-model resolver and do not substitute a newer model. Search for and fetch current guidance for that exact model. A GPT-5.4 migration must not load GPT-5.6 guidance or references.
- **Unspecified, latest, current, or default target:** Search for and fetch `https://developers.openai.com/api/docs/guides/latest-model` first. Use the corresponding `latest-model.md` metadata only when dynamic migration resolution is needed, then run the platform-specific resolver below and preserve its returned model and exact guide URLs.
- **Latest/current/default prompting:** Follow the dynamic-target route, then use the returned prompting guide. Do not run the resolver for explicitly named-model prompting.
- **Pure model selection:** Use `references/model-selection.md` instead. Do not run the resolver.

For POSIX shells, invoke the resolver through `sh`, without assuming an executable bit:

```sh
sh <skill-dir>/scripts/resolve-latest-model-info
```

On Windows, use the CommonJS entry point with Node.js 18 or newer:

```text
node <skill-dir>\scripts\resolve-latest-model-info.cjs
```

If the Windows Node runtime is unavailable and `load_workspace_dependencies` is callable, use its returned runtime and retry once. Do not execute the extensionless POSIX wrapper directly on Windows.

Do not suppress or redirect resolver stdout. Success requires JSON with nonempty `model`, `migrationGuideUrl`, and `promptingGuideUrl` fields. If the command fails or any required field is missing, retry the platform-specific command once, then fall back to current official documentation and finally disclosed bundled references.

## Retrieve only the guidance this request needs

Treat returned guide URLs as opaque: fetch those exact URLs without deriving, substituting, or appending a model query. Use an available official documentation or first-party-domain capability to open and read the relevant official page. Retry the exact guide URL when its response contains only a title or no substantive body.

- Fetch `migrationGuideUrl` for a requested migration or upgrade plan.
- Fetch `promptingGuideUrl` only when the user asks for prompting guidance or the migration requires a prompt change. Extract only `## Prompting Best Practices` through the next H2 heading.
- For explicitly named-model prompting, fetch that model's official prompting guidance and extract only `## Prompting Best Practices` through the next H2 heading. Do not load a migration reference or run the resolver.
- For an actual GPT-5.6-family migration or implementation plan, fetch `https://developers.openai.com/api/docs/guides/upgrading-to-gpt-5p6-sol`. For specifically requested GPT-5.6 prompting, fetch `https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6`. Read `references/upgrading-to-gpt-5p6-sol.md` only when fetched official guidance does not resolve needed compatibility gates, scoped code changes, tier-aware routing, validation, or other migration-specific judgment. Never load it for documentation-only questions about model tiers, the family alias, Pro mode, reasoning effort, or current guidance when the fetched official documentation already answers them.
- Read `references/prompting-guide.md` only when prompting guidance or prompt changes are actually needed and current official guidance is unavailable.
- Read `references/upgrade-guide.md` only when current official migration guidance is unavailable. Disclose when a bundled fallback was used.

## Keep implementation changes scoped

Change active model defaults and directly related prompt surfaces only when the user requested that work. Update registries, model pickers, capability metadata, routing, pricing, or tests only when they are in scope and current official documentation verifies the relevant values.

Preserve each workload's cost, latency, quality, reasoning, tool, endpoint, and output-contract role. Do not collapse a tiered router into one flagship model, replace intentionally pinned fallbacks, or rewrite historical examples, fixtures, eval baselines, provider comparisons, or unrelated SDK and authentication configuration.

If a safe migration requires an endpoint change, request-schema change, tool-handler change, or other implementation outside the requested scope, report the exact compatibility blocker and smallest follow-up instead of silently changing behavior.
