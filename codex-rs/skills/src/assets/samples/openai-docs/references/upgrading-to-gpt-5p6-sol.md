# Upgrading to GPT-5.6 Sol

Use this guide when the user asks to migrate an existing OpenAI API integration, repository, prompt stack, agent, model router, or model picker to GPT-5.6 Sol or the GPT-5.6 family.

The default explicit target is `gpt-5.6-sol`. The alias `gpt-5.6` routes to Sol; use it only when the repository intentionally prefers family aliases. Do not treat every old model usage as a Sol candidate: GPT-5.6 is a family with different cost, latency, context, and quality roles.

Before changing code, use the OpenAI Docs MCP to fetch the current live GPT-5.6 model guidance:

https://developers.openai.com/api/docs/guides/model-guidance?model=gpt-5.6

For prompt changes, also read only the `## Prompting Best Practices` section from:

https://developers.openai.com/api/docs/guides/model-guidance?model=gpt-5.6#prompting-best-practices

Treat live docs as canonical for current model IDs, parameters, limits, pricing, and feature availability. This file supplies migration judgment: where to look, what can break, what to preserve, what not to adopt automatically, and how to validate the result.

## Core principle

Do not perform a blind model-string replacement.

First preserve the behavior, latency class, cost class, reasoning level, endpoint contract, tool semantics, cache behavior, and output contract of each usage site. Then make the smallest safe migration. Adopt new GPT-5.6 capabilities only when they solve a measured problem or the user explicitly asks for them.

A model upgrade alone does not authorize adding reasoning fields, changing request schemas, or rewriting tests. Only add explicit reasoning when the old effective behavior is established and omission would change behavior on GPT-5.6.

The main 5.6 migration hazards are:

- choosing Sol for workloads that were intentionally mini, nano, low-cost, or latency-sensitive;
- inheriting 5.6's default `medium` reasoning where the old effective effort was `none`;
- using Chat Completions with function tools without explicitly setting effective reasoning to `none`;
- losing prompt-cache hits when a stable prefix is followed by a changing suffix;
- increasing image or PDF input tokens because omitted or `auto` detail behaves differently;
- applying new cache, persisted-reasoning, Pro, Programmatic Tool Calling, or multi-agent fields to routes that do not support them;
- updating model strings but forgetting registries, allowlists, pricing metadata, capability flags, tests, and UI model pickers.

## Migration posture

Classify every usage site before editing:

1. `simple Sol migration`
   - One flagship model usage.
   - Same endpoint and request shape can remain.
   - Reasoning effort is explicit or its old effective value is known.
   - No cache, vision, file, tool, or parser behavior needs implementation changes.
2. `tier-aware family migration`
   - The repository exposes multiple model roles, model choices, fallbacks, routers, pricing data, or capability metadata.
   - Map each role to Sol, Terra, or Luna instead of replacing everything with Sol.
3. `compatibility migration`
   - The safe move requires parameter, endpoint, cache, state, tool-loop, or multimodal-detail changes.
   - Make these changes only when implementation work is inside the user's requested scope. Otherwise report the exact blocker and smallest follow-up.
4. `prompt migration`
   - The API shape can remain, but representative traces show a prompt-specific regression.
   - Make a surgical prompt edit tied to that failure; do not rewrite a working prompt stack wholesale.
   - When the task is to update prompting guidance, edit the directly tied prompt surface only. Do not modify runtime request code, model schemas, or tests unless the prompt change requires it.
5. `optional feature adoption`
   - Pro mode, persisted reasoning, explicit caching, Programmatic Tool Calling, or multi-agent behavior is being added deliberately.
   - Keep this separate from the baseline migration so its effect can be measured.
6. `leave unchanged`
   - Historical examples, documentation about old models, snapshots, fixtures, eval baselines, comparison code, intentionally pinned fallbacks, unsupported providers, or ambiguous usages.

When intent is unclear, prefer leaving a usage unchanged and list it for confirmation over silently changing its role.

## Inventory before editing

Search for more than literal model IDs. Inventory:

- model strings, aliases, environment variables, CLI flags, config defaults, and deployment settings;
- SDK calls to Responses, Chat Completions, Batch, or provider adapters;
- reasoning settings, token budgets, sampling settings, and latency timeouts;
- function tools, hosted tools, structured outputs, response parsers, and replay logic;
- system, developer, user, and tool-description prompts tied to each usage;
- routers, fallbacks, model allowlists, enums, regexes, validation schemas, and capability maps;
- model picker UI, display labels, descriptions, context limits, pricing metadata, and provider catalogs;
- prompt-cache keys, retention options, stable-prefix construction, and cache metrics;
- image, PDF, file, OCR, and computer-use inputs;
- tests, fixtures, snapshots, evals, analytics labels, billing tables, and docs.

When changing a default model, search every active default surface: runtime config, environment/config files, setup docs, tests, CLI defaults, and deployment examples. Update them together.

For each usage site, record:

- source model and why it appears to be used;
- endpoint and SDK/client surface;
- prompt surface;
- effective reasoning effort, including defaults;
- latency, cost, context, and quality role;
- tools, structured outputs, caching, state replay, and multimodal inputs;
- downstream parsers or user-visible contracts;
- migration class and validation plan.

## Choose the target model by role

Use this as a starting map, then validate against the repository's workload:

| Existing role | Starting GPT-5.6 target | Reason |
| --- | --- | --- |
| Unsuffixed GPT-5 flagship, GPT-5.5, or GPT-5.4 flagship | `gpt-5.6-sol` | Sol is the flagship-equivalent tier. |
| Mini model, balanced lower-cost route, or medium-throughput worker | `gpt-5.6-terra` | Terra is the mini-like tier. |
| Nano model, classification, extraction, routing, high-volume, or strict-latency route | `gpt-5.6-luna` | Luna is the nano-like tier. |
| GPT-4.1 or GPT-4o latency-sensitive flow | Evaluate Luna and Terra first; use Sol only if quality requires it | A flagship replacement can change latency and cost materially. |
| Reasoning-heavy or hardest quality-first flow | Start with Sol at the old effective effort | Preserve the reasoning contract before tuning. |
| Old Pro usage | Sol plus `reasoning.mode: "pro"`, only if the user wants Pro behavior | GPT-5.6 Pro is a mode, not a separate model slug. |
| Router, fallback, or model picker | Add the family by role | Do not collapse a multi-model design into Sol. |
| Third-party or provider-specific model | Leave unchanged unless the user explicitly requests provider migration | Model-name similarity is not a safe mapping. |

Important limits to check in live docs:

- Sol and Terra have roughly 1.05M context and 128K maximum output.
- Luna has a smaller 400K context and 128K maximum output.
- Sol and Terra long-context requests above 272K input tokens can change pricing for the full request.

Do not invent prices, limits, or capability flags. Fetch them from current docs before updating a registry or UI.

For model pickers and registries, preserve existing model entries by default. Add GPT-5.6 Sol, Terra, and Luna as new options unless the user explicitly asks to replace or remove older models. Do not invent pricing, context limits, capabilities, or metadata unless confirmed from canonical docs.

If using the `gpt-5.6` alias, record the returned `response.model` during validation. Do not assume an alias and an explicit Sol slug appear identically in dashboards, rate-limit configuration, analytics, or billing metadata.

## Preserve effective reasoning before tuning

GPT-5.6 supports `none`, `low`, `medium`, `high`, `xhigh`, and `max`. If omitted, GPT-5.6 defaults to `medium`.

This is a behavioral migration hazard:

- GPT-5.5 commonly defaulted to `medium`.
- GPT-5.4, mini, and nano usages commonly defaulted to `none`.
- A previously omitted setting can therefore become slower, more expensive, and incompatible with Chat Completions function tools after the model swap.

For each usage:

1. If effort is explicit, preserve it for the first 5.6 run when supported.
2. If effort is omitted and the old effective default is known, add it explicitly only when GPT-5.6's omitted default would change behavior. If both old and new omitted defaults are the same, keep it omitted.
3. If the old effective value is unknown, do not guess. Flag it and compare the old behavior with 5.6 at the likely baseline.
4. After the baseline passes, test the same setting and one lower on representative tasks.
5. Use `xhigh` or `max` only for hard quality-first workloads where evals show a meaningful gain.

Do not globally recommend `max`. Before increasing effort, check whether the actual failure is a missing success criterion, dependency rule, tool-routing rule, state-replay bug, or validation loop.

Use the field shape that belongs to the endpoint.

Responses:

```json
{
  "model": "gpt-5.6-sol",
  "reasoning": { "effort": "none" }
}
```

Chat Completions:

```json
{
  "model": "gpt-5.6-sol",
  "reasoning_effort": "none"
}
```

## Chat Completions and function tools

This is the most important endpoint-specific check.

For GPT-5.6, function tools in Chat Completions are compatible only with effective reasoning `none`. Reasoning with tools should use the Responses API.

Because GPT-5.6 defaults to `medium`, this combination is unsafe:

```json
{
  "model": "gpt-5.6-luna",
  "tools": [{ "type": "function", "function": { "...": "..." } }]
}
```

For a latency-sensitive Chat Completions flow that must keep function tools, explicitly preserve `none`:

```json
{
  "model": "gpt-5.6-luna",
  "reasoning_effort": "none",
  "tools": [{ "type": "function", "function": { "...": "..." } }]
}
```

If the application needs both reasoning and tools:

- migrate that flow to Responses when implementation changes are in scope;
- otherwise report it as a compatibility blocker;
- do not hide the incompatibility by removing tools, dropping required reasoning, or changing the workload's behavior without approval.

If the live API rejects the intended `none` path, treat it as a current API compatibility issue and report the exact request and error rather than inventing a workaround.

## Responses API and conversation state

Prefer Responses for reasoning, tools, multi-turn agents, and new 5.6 capabilities.

For ordinary multi-turn Responses calls, preserve the repository's existing state strategy. Do not add persisted reasoning merely because it exists.

If deliberately enabling persisted reasoning:

- use `reasoning.context: "all_turns"` only when the objective and assumptions remain stable;
- prefer `previous_response_id` when the server can carry state;
- when replaying manually, preserve every prior user input and every relevant output item, not only assistant text;
- with `store: false` or ZDR, request and replay `reasoning.encrypted_content`;
- use current-turn behavior when old reasoning may be stale or misleading.

For manual replay, preserve item types, IDs, call IDs, caller metadata, and assistant phase values exactly. Incomplete replay can silently reduce quality or break tool continuation.

## Prompt caching

Do not assume old cache-hit behavior survives the model swap.

GPT-5.6 implicit caching places a managed breakpoint near the latest user or tool message and no longer relies on 128-token rounding. A prompt with a large stable prefix followed by a changing suffix can therefore lose cache hits even when the stable prefix itself has not changed.

Audit:

- large reusable system/developer prompts;
- dynamic suffixes appended to otherwise stable prompts;
- changing timestamps, request IDs, user-specific values, or tool lists in the prefix;
- cache keys, retention settings, and cache dashboards;
- token accounting that assumes reads only and ignores writes.

Migration rules:

- keep reusable prefixes stable;
- do not churn large system prompts unnecessarily;
- compare old and new `cached_tokens`, `cache_write_tokens`, latency, and cost;
- use explicit cache breakpoints only when a measured workload has a stable boundary that implicit caching misses;
- do not globally convert every prompt to explicit caching;
- do not send 5.6-only cache fields to older routes in a mixed-model system.

When old and GPT-5.6 routes share a request builder, isolate GPT-5.6-only fields instead of applying them globally.

The new top-level request shape uses `prompt_cache_options`, for example:

```json
{
  "prompt_cache_options": {
    "mode": "explicit",
    "ttl": "30m"
  }
}
```

Place explicit breakpoints at the actual stable rendered boundary using `prompt_cache_breakpoint`. Preserve `prompt_cache_key` when the application already uses it. Treat the older `prompt_cache_retention` shape as deprecated and verify the live docs before rewriting it.

Cache writes cost more than ordinary uncached input, so a lower hit rate can be both slower and more expensive.

## Images, PDFs, files, and long context

GPT-5.6 can change token and latency behavior without any prompt change:

- for image inputs, omitted or `auto` image detail can preserve original dimensions;
- for PDF/file inputs in Responses, omitted or `input_file.detail: "auto"` can use high page-image detail;
- Chat Completions file inputs do not expose the same detail control;
- long-context Sol and Terra requests can cross pricing thresholds;
- Luna's smaller context can break workloads that fit in Sol or Terra.

For multimodal or long-context usages:

1. Measure input tokens and latency before and after.
2. Make detail explicit when cost or latency matters.
3. Resize images or use lower detail when the task does not need original spatial precision.
4. Keep original/high detail for dense, coordinate-sensitive, OCR, localization, or visual-inspection tasks where it materially improves quality.
5. Test worst-case context lengths, not only typical requests.

Do not claim a capability was removed based only on a missing metadata flag. Verify against current docs and a representative request.

## Structured outputs, parsers, and tool contracts

Keep output contracts explicit:

- preserve JSON schemas, required fields, enums, refusal handling, and parser expectations;
- preserve tool names, parameter schemas, call IDs, and retry behavior;
- keep citations, evidence fields, or native artifacts when downstream consumers require them;
- validate that the final answer still satisfies the contract, not merely that a tool call succeeded.

Do not fix a failing migration by weakening a schema, deleting required behavior, removing routes, dropping tools, or changing business logic unless the user explicitly asked for that product change.

## Optional: Pro mode

Do not enable Pro mode during a baseline migration unless the old usage was Pro-like or the user explicitly asks for it.

GPT-5.6 Pro uses the base model with a reasoning mode:

```json
{
  "model": "gpt-5.6-sol",
  "reasoning": {
    "mode": "pro",
    "effort": "medium"
  }
}
```

Rules:

- use Responses, not Chat Completions;
- do not search for or invent a separate `gpt-5.6-pro` slug;
- supported Pro efforts begin at `medium`;
- mode and effort are separate decisions;
- compare task quality, total latency, and actual billed token usage against standard mode.

If migrating a legacy Pro slug, make the mode change explicit and evaluate it separately from ordinary Sol migration.

## Optional: Programmatic Tool Calling

Programmatic Tool Calling is not a required part of moving to GPT-5.6. Add it only when code can reduce large structured intermediate results before they return to model context.

Good candidates:

- bounded read-only filtering, joining, sorting, ranking, deduplication, and aggregation;
- batching many similar records;
- repeated deterministic validation;
- map-reduce style retrieval with a compact result schema.

Poor candidates:

- one direct tool call;
- adaptive workflows where each result changes the next decision;
- write, approval, or side-effecting flows;
- citation-heavy or native-artifact flows;
- semantic judgment that should remain visible to the model.

Request-shape requirements:

```json
{
  "tools": [
    { "type": "programmatic_tool_calling" },
    {
      "type": "function",
      "name": "lookup_records",
      "allowed_callers": ["programmatic"]
    }
  ]
}
```

Do not nest `programmatic_tool_calling` under another `tools` property. When enabled, the host must handle `program`, program-issued `function_call`, `function_call_output`, and `program_output` items. Preserve the original `call_id` and `caller` when returning function results.

Constrain the stage, eligible read-only tools, output schema, retry limit, and handoff back to direct judgment. Validate the final user-visible answer; a correct program result can still become an incorrect final answer.

## Optional: multi-agent beta

Do not enable multi-agent behavior during a baseline migration unless the application already has a clear parallelizable workflow and the user asks for it.

Enabling it requires:

- the `OpenAI-Beta: responses_multi_agent=v1` header;
- `multi_agent: { "enabled": true, "max_concurrent_subagents": 3 }`;
- handling `multi_agent_call`, `multi_agent_call_output`, and `agent_message` items;
- executing ordinary developer-defined function calls from any agent and returning all required outputs;
- preserving new items for replay and tracing;
- checking incompatibilities with compaction, reasoning summaries, and tool-call limits in current docs.

Cap concurrency. Do not let a migration task create unbounded subagents, duplicate work, or finish without a final synthesis.

## Prompt migration judgment

After the model and API baseline is working, run representative traces before editing prompts. Change prompts only for measured failures.

For GPT-5.6, prefer:

- shorter, outcome-oriented prompts;
- explicit success criteria, dependencies, stopping conditions, and completion boundaries;
- preserved user-provided values;
- decision criteria for implicit choices instead of universal defaults or keyword maps;
- explicit autonomy and permission boundaries;
- explicit tool routing, resource links, breadcrumbs, and expected tool choice;
- staged plans, current-layer awareness, and concise handoffs for long work;
- real validation before declaring completion.

Avoid:

- generic `be brief`, `be thorough`, or `think step by step` instructions;
- blanket language instructions that can cause unwanted language switching;
- repeating `ask first` until safe local work becomes blocked;
- giant prompt rewrites that make the source of a regression impossible to identify;
- telling the model to minimize tool loops when correctness, evidence, or required validation needs more work.

For coding or agentic migrations, add concrete preservation and verification rules:

```
Preserve existing functionality, routes, outputs, and user-visible behavior.
Do not delete or disable required behavior merely to make the build pass.
Before finishing, run the relevant build, tests, type checks, render or smoke
checks, and report the evidence.
```

For long-running work, define the current layer: research, design, implementation, review, or external coordination. Do not let the model silently move to another layer.

## Upgrade workflow

1. Fetch current live 5.6 docs and the Prompting Best Practices section.
2. Inventory every usage site and its adjacent prompt, config, registry, parser, and test surfaces.
3. Classify each usage by role and migration class.
4. Choose Sol, Terra, or Luna by the existing workload's role.
5. Preserve the old effective reasoning effort explicitly.
6. Run the compatibility gates:
   - endpoint and SDK support;
   - Chat Completions plus function tools;
   - cache topology and cache fields;
   - context length and long-context cost;
   - image, PDF, and file detail;
   - structured outputs and parsers;
   - Responses state replay and tool continuation;
   - mixed-model routing and unsupported new fields.
7. Apply the smallest safe model, config, registry, and prompt changes.
8. Do not add optional Pro, persisted reasoning, PTC, explicit caching, or multi-agent behavior unless needed and measurable.
9. Run existing tests and representative evals.
10. Report changed, unchanged, blocked, and confirmation-needed sites separately.

## Validation matrix

Prefer a controlled comparison:

1. old model + old prompt + old settings;
2. GPT-5.6 target + same prompt + preserved effective reasoning;
3. GPT-5.6 target + same prompt + one lower effort;
4. GPT-5.6 target + the smallest prompt or API fix required by a measured failure;
5. optional feature treatment, isolated from the baseline.

Measure what matters for the workflow:

- task success and user-visible quality;
- structured-output validity and parser success;
- tool choice, tool arguments, retries, loop count, and completion rate;
- TTFT, end-to-end latency, timeout rate, and concurrency behavior;
- input, output, reasoning, cached, and cache-write tokens;
- cost per successful task;
- long-context, compaction, and replay behavior;
- image/PDF token use and visual/OCR accuracy;
- completeness, preserved behavior, citations, and validation evidence.

For model routers and pickers, test at least one representative workload for each role. Verify that the cheapest or fastest tier is not accidentally used for quality-critical work and that Sol is not accidentally used for every workload.

## Required final report

Return:

- `Current usage inventory`: each model site, endpoint, role, prompt surface, and old effective reasoning.
- `Target mapping`: Sol, Terra, Luna, unchanged, or confirmation-needed, with the reason.
- `Changes made`: model strings, reasoning settings, prompts, registries, metadata, tests, and API-shape changes.
- `Compatibility checks`: Chat Completions/tools, caching, state replay, multimodal detail, context/cost, schemas, and mixed-model routing.
- `Prompt changes`: each surgical edit and the failure mode it addresses.
- `Validation`: commands, evals, traces, before/after measurements, and remaining gaps.
- `Unchanged sites`: historical, pinned, ambiguous, or intentionally role-specific usages.
- `Blockers and open questions`: exact issue, why it is unsafe to guess, and the smallest next step.

Never say the migration is complete merely because model strings changed. It is complete only when the affected behavior and contracts have been validated or the remaining gaps are stated explicitly.
