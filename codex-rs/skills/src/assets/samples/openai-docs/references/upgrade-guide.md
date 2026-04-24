# Upgrading to GPT-5.4

Use this guide when the user explicitly asks to upgrade an existing integration to GPT-5.4. Pair it with current OpenAI docs lookups. The default target string is `gpt-5.4`.

## Freshness check

Before applying this bundled guide, run `node scripts/resolve-latest-model-info.js` from the OpenAI Docs skill directory.

- If the command returns `modelSlug: "gpt-5p4"`, continue with this bundled guide and use `references/prompting-guide.md` when prompt updates are needed.
- If the command returns a different `modelSlug`, fetch both the returned `migrationGuideUrl` and `promptingGuideUrl` and use them as the current source of truth instead of the bundled references.
- If the command fails, the metadata is missing, or either remote guide cannot be fetched, continue with the bundled fallback references and say the remote freshness check was unavailable.

## Upgrade posture

Upgrade with the narrowest safe change set:

- replace the model string first
- update only the prompts that are directly tied to that model usage
- prefer prompt-only upgrades when possible
- if the upgrade would require API-surface changes, parameter rewrites, tool rewiring, or broader code edits, mark it as blocked instead of stretching the scope

## Upgrade workflow

1. Inventory current model usage.
   - Search for model strings, client calls, and prompt-bearing files.
   - Include inline prompts, prompt templates, YAML or JSON configs, Markdown docs, and saved prompts when they are clearly tied to a model usage site.
2. Pair each model usage with its prompt surface.
   - Prefer the closest prompt surface first: inline system or developer text, then adjacent prompt files, then shared templates.
   - If you cannot confidently tie a prompt to the model usage, say so instead of guessing.
3. Classify the source model family.
   - Common buckets: `gpt-4o` or `gpt-4.1`, `o1` or `o3` or `o4-mini`, early `gpt-5`, later `gpt-5.x`, or mixed and unclear.
4. Decide the upgrade class.
   - `model string only`
   - `model string + light prompt rewrite`
   - `blocked without code changes`
5. Run the no-code compatibility gate.
   - Check whether the current integration can accept `gpt-5.4` without API-surface changes or implementation changes.
   - For long-running Responses or tool-heavy agents, check whether `phase` is already preserved or round-tripped when the host replays assistant items or uses preambles.
   - If compatibility depends on code changes, return `blocked`.
   - If compatibility is unclear, return `unknown` rather than improvising.
6. Recommend the upgrade.
   - Default replacement string: `gpt-5.4`
   - Keep the intervention small and behavior-preserving.
7. Deliver a structured recommendation.
   - `Current model usage`
   - `Recommended model-string updates`
   - `Starting reasoning recommendation`
   - `Prompt updates`
   - `Phase assessment` when the flow is long-running, replayed, or tool-heavy
   - `No-code compatibility check`
   - `Validation plan`
   - `Launch-day refresh items`

Output rule:

- Always emit a starting `reasoning_effort_recommendation` for each usage site.
- If the repo exposes the current reasoning setting, preserve it first unless the source guide says otherwise.
- If the repo does not expose the current setting, use the source-family starting mapping instead of returning `null`.

## Upgrade outcomes

### `model string only`

Choose this when:

- the existing prompts are already short, explicit, and task-bounded
- the workflow is not strongly research-heavy, tool-heavy, multi-agent, batch or completeness-sensitive, or long-horizon
- there are no obvious compatibility blockers

Default action:

- replace the model string with `gpt-5.4`
- keep prompts unchanged
- validate behavior with existing evals or spot checks

### `model string + light prompt rewrite`

Choose this when:

- the old prompt was compensating for weaker instruction following
- the workflow needs more persistence than the default tool-use behavior will likely provide
- the task needs stronger completeness, citation discipline, or verification
- the upgraded model becomes too verbose or under-complete unless instructed otherwise
- the workflow is research-heavy and needs stronger handling of sparse or empty retrieval results
- the workflow is coding-oriented, tool-heavy, or multi-agent, but the existing API surface and tool definitions can remain unchanged

Default action:

- replace the model string with `gpt-5.4`
- add one or two targeted prompt blocks
- read `references/prompting-guide.md` to choose the smallest prompt changes that preserve the intended behavior and take advantage of relevant model-specific guidance
- avoid broad prompt cleanup unrelated to the upgrade
- for research workflows, default to `research_mode` + `citation_rules` + `empty_result_recovery`; add `tool_persistence_rules` when the host already uses retrieval tools
- for dependency-aware or tool-heavy workflows, default to `tool_persistence_rules` + `dependency_checks` + `verification_loop`; add `parallel_tool_calling` only when retrieval steps are truly independent
- for coding or terminal workflows, default to `terminal_tool_hygiene` + `verification_loop`
- for multi-agent support or triage workflows, default to at least one of `tool_persistence_rules`, `completeness_contract`, or `verification_loop`
- for long-running Responses agents with preambles or multiple assistant messages, explicitly review whether `phase` is already handled; if adding or preserving `phase` would require code edits, mark the path as `blocked`
- do not classify a coding or tool-using Responses workflow as `blocked` just because the visible snippet is minimal; prefer `model string + light prompt rewrite` unless the repo clearly shows that a safe GPT-5.4 path would require host-side code changes

### `blocked`

Choose this when:

- the upgrade appears to require API-surface changes
- the upgrade appears to require parameter rewrites or reasoning-setting changes that are not exposed outside implementation code
- the upgrade would require changing tool definitions, tool handler wiring, or schema contracts
- you cannot confidently identify the prompt surface tied to the model usage

Default action:

- do not improvise a broader upgrade
- report the blocker and explain that the fix is out of scope for this guide

## No-code compatibility checklist

Before recommending a no-code upgrade, check:

1. Can the current host accept the `gpt-5.4` model string without changing client code or API surface?
2. Are the related prompts identifiable and editable?
3. Does the host depend on behavior that likely needs API-surface changes, parameter rewrites, or tool rewiring?
4. Would the likely fix be prompt-only, or would it need implementation changes?
5. Is the prompt surface close enough to the model usage that you can make a targeted change instead of a broad cleanup?
6. For long-running Responses or tool-heavy agents, is `phase` already preserved if the host relies on preambles, replayed assistant items, or multiple assistant messages?

If item 1 is no, items 3 through 4 point to implementation work, or item 6 is no and the fix needs code changes, return `blocked`.

If item 2 is no, return `unknown` unless the user can point to the prompt location.

Important:

- Existing use of tools, agents, or multiple usage sites is not by itself a blocker.
- If the current host can keep the same API surface and the same tool definitions, prefer `model string + light prompt rewrite` over `blocked`.
- Reserve `blocked` for cases that truly require implementation changes, not cases that only need stronger prompt steering.

## Scope boundaries

This guide may:

- update or recommend updated model strings
- update or recommend updated prompts
- inspect code and prompt files to understand where those changes belong
- inspect whether existing Responses flows already preserve `phase`
- flag compatibility blockers

This guide may not:

- move Chat Completions code to Responses
- move Responses code to another API surface
- rewrite parameter shapes
- change tool definitions or tool-call handling
- change structured-output wiring
- add or retrofit `phase` handling in implementation code
- edit business logic, orchestration logic, or SDK usage beyond a literal model-string replacement

If a safe GPT-5.4 upgrade requires any of those changes, mark the path as blocked and out of scope.

## Validation plan

- Validate each upgraded usage site with existing evals or realistic spot checks.
- Check whether the upgraded model still matches expected latency, output shape, and quality.
- If prompt edits were added, confirm each block is doing real work instead of adding noise.
- If the workflow has downstream impact, add a lightweight verification pass before finalization.

## Launch-day refresh items

When final GPT-5.4 guidance changes:

1. Replace release-candidate assumptions with final GPT-5.4 guidance where appropriate.
2. Re-check whether the default target string should stay `gpt-5.4` for all source families.
3. Re-check any prompt-block recommendations whose semantics may have changed.
4. Re-check research, citation, and compatibility guidance against the final model behavior.
5. Re-run the same upgrade scenarios and confirm the blocked-versus-viable boundaries still hold.
