# Upgrading to GPT-6 Astra

Use this guide when the user asks to migrate an existing OpenAI API integration, repository, prompt stack, agent, model router, or model picker to GPT-6 Astra.

The default explicit target is `gpt-6-astra`. Verify the `gpt-6` family alias's currently documented routing and availability before using it. Do not treat every old model usage as an Astra candidate: retain Terra for balanced work and Luna as the primary faster or cheaper model.

Before changing code, retrieve the current live GPT-6 model guidance using already-callable official documentation search and fetch, or immediately use official-domain web search and fetch:

https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md

For prompt changes, also read only the `## Prompting best practices` section from:

https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices

Treat live docs as canonical for current model IDs, parameters, limits, pricing, and feature availability. The skill-specific workflow below covers repository inspection, scope preservation, and validation. The fallback after it includes all non-prompting guidance, including access notices, examples, caveats, and `## Migration quickstart`; the full prompting section is in `references/prompting-guide.md`. When refreshing, preserve all guide content unless it is specific to the website rather than useful to the skill, and record any omission. Remove website metadata and component markup while retaining their readable content. Resolve site-relative links against `https://developers.openai.com` and section-only links against the canonical model-guide URL above.

## Core principle

Do not perform a blind model-string replacement.

First preserve the behavior, latency class, cost class, reasoning level, endpoint contract, tool semantics, cache behavior, and output contract of each usage site. Then make the smallest safe migration. Adopt new GPT-6 capabilities only when they solve a measured problem or the user explicitly asks for them.

A model upgrade alone does not authorize adding reasoning fields, changing request schemas, or rewriting tests. Set a supported reasoning effort explicitly to preserve the source model's effective behavior; verify omitted defaults rather than guessing.

## Migration posture

Classify every usage site before editing:

1. `simple Astra migration`
   - One flagship model usage.
   - Same endpoint and request shape can remain.
   - Reasoning effort is explicit or its old effective value is known.
   - No cache, vision, file, tool, or parser behavior needs implementation changes.
2. `tier-aware family migration`
   - The repository exposes multiple model roles, model choices, fallbacks, routers, pricing data, or capability metadata.
   - Map each role to Astra, Terra, or Luna instead of replacing everything with Astra.
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

| Existing role | Starting target | Reason |
| --- | --- | --- |
| GPT-5.6 Sol or an earlier flagship | `gpt-6-astra` | Astra is the flagship-equivalent tier. |
| Balanced quality, latency, and cost | `gpt-5.6-terra` | Terra is the balanced option. |
| Faster or cheaper work, classification, extraction, routing, high-volume, or strict-latency route | `gpt-5.6-luna` | Luna is the primary speed and cost option. |
| GPT-4.1 or GPT-4o latency-sensitive flow | Start with Luna; evaluate Terra or Astra if quality requires it | A flagship replacement can change latency and cost materially. |
| Reasoning-heavy or hardest quality-first flow | Start with Astra at the old effective effort | Preserve the reasoning contract before tuning. |
| Router, fallback, or model picker | Add the family by role | Do not collapse a multi-model design into Astra. |
| Third-party or provider-specific model | Leave unchanged unless the user explicitly requests provider migration | Model-name similarity is not a safe mapping. |

Important limits to check in live docs:

- Each model's context window and maximum output.
- Long-context pricing thresholds for each route.
- Token pricing for GPT-6.

Do not invent prices, limits, or capability flags. Fetch them from current docs before updating a registry or UI.

For model pickers and registries, preserve existing model entries by default. Add GPT-6 Astra and retain the existing Terra and Luna options unless the user explicitly asks to replace or remove them. Do not invent pricing, context limits, capabilities, or metadata unless confirmed from canonical docs.

If using the `gpt-6` alias, record the returned `response.model` during validation. Do not assume an alias and an explicit Astra slug appear identically in dashboards, rate-limit configuration, analytics, or billing metadata.

## Structured outputs, parsers, and tool contracts

Keep output contracts explicit:

- preserve JSON schemas, required fields, enums, refusal handling, and parser expectations;
- preserve tool names, parameter schemas, call IDs, and retry behavior;
- keep citations, evidence fields, or native artifacts when downstream consumers require them;
- validate that the final answer still satisfies the contract, not merely that a tool call succeeded.

Do not fix a failing migration by weakening a schema, deleting required behavior, removing routes, dropping tools, or changing business logic unless the user explicitly asked for that product change.

## Prompt migration judgment

After the model and API baseline is working, run representative traces before editing prompts. Change prompts only for measured failures. Read `references/prompting-guide.md` for the exact canonical prompting section when prompt changes are needed.

## Upgrade workflow

1. Fetch current live GPT-6 docs. Fetch the Prompting Best Practices section only when prompt changes are needed.
2. Inventory every usage site and its adjacent prompt, config, registry, parser, and test surfaces.
3. Classify each usage by role and migration class.
4. Choose Astra, Terra, or Luna by the existing workload's role.
5. Preserve the old effective reasoning effort explicitly when supported; follow the canonical migration guidance for unsupported settings.
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
8. Do not add optional Pro, persisted reasoning, PTC, explicit caching, async tools, or multi-agent behavior unless needed and measurable.
9. Run existing tests and representative evals.
10. Report changed, unchanged, blocked, and confirmation-needed sites separately.

## Validation matrix

Prefer a controlled comparison:

1. old model + old prompt + old settings;
2. GPT-6 target + same prompt + preserved effective reasoning;
3. GPT-6 target + same prompt + one lower supported effort;
4. GPT-6 target + the smallest prompt or API fix required by a measured failure;
5. optional feature treatment, isolated from the baseline.

Measure what matters for the workflow:

- task success and user-visible quality;
- structured-output validity and parser success;
- tool choice, tool arguments, retries, loop count, and completion rate;
- TTFT, end-to-end latency, timeout rate, and concurrency behavior;
- input, output, reasoning, cached, and cache-write tokens;
- total cost per successful task;
- long-context, compaction, and replay behavior;
- image/PDF token use and visual/OCR accuracy;
- completeness, preserved behavior, citations, and validation evidence.

For model routers and pickers, test at least one representative workload for each role. Verify that the cheapest or fastest tier is not accidentally used for quality-critical work and that Astra is not accidentally used for every workload.

## Required final report

Return:

- `Current usage inventory`: each model site, endpoint, role, prompt surface, and old effective reasoning.
- `Target mapping`: Astra, Terra, Luna, unchanged, or confirmation-needed, with the reason.
- `Changes made`: model strings, reasoning settings, prompts, registries, metadata, tests, and API-shape changes.
- `Compatibility checks`: Chat Completions/tools, caching, state replay, multimodal detail, context/cost, schemas, and mixed-model routing.
- `Prompt changes`: each surgical edit and the failure mode it addresses.
- `Validation`: commands, evals, traces, before/after measurements, and remaining gaps.
- `Unchanged sites`: historical, pinned, ambiguous, or intentionally role-specific usages.
- `Blockers and open questions`: exact issue, why it is unsafe to guess, and the smallest next step.

Never say the migration is complete merely because model strings changed. It is complete only when the affected behavior and contracts have been validated or the remaining gaps are stated explicitly.

## Introduction

GPT-6 Astra is our most intelligent model yet, with state-of-the-art performance in computer use, browsing, software engineering, science, and professional work. It excels at carrying out multistep workflows across code, browsers, and professional software. In <a href="https://openai.com/index/gpt-6-astra/" target="_blank" rel="noopener noreferrer">several evaluations</a>, Astra achieves stronger results while using substantially fewer output tokens—delivering a lower estimated API cost per task than earlier models despite its higher per-token pricing.

GPT-6 Astra is also our most aligned model yet. It excels at exercising care, respecting task boundaries, and communicating transparently. When instructions leave room for interpretation, it uses the context it has to fill in routine gaps and asks focused questions when the answer could change the outcome. It incorporates new requirements, changes course when asked, and answers side questions without losing track of the broader task.

To build with Astra, set `model` to `gpt-6-astra` in a [Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses) request.

## What's new

- **Async tool calling:** GPT-6 Astra can continue reasoning, call other tools, or answer independent parts of a request while your application runs a tool. Set `async: true` on a function or custom tool and return its result when ready using the original `call_id`. Your application still executes the tool and manages pending work. See [Async tool calling](https://developers.openai.com/api/docs/guides/async-tool-calling) for basic usage and a developer-defined wait-tool pattern.
- **Mid-turn steering:** Send additional user instructions while GPT-6 Astra is working, such as a correction or a change in requirements. Over a WebSocket connection, the Responses API preserves completed work and includes the update in a continuation. See [Mid-turn steering](https://developers.openai.com/api/docs/guides/steering) for the event flow and tool-result handling.
- **Change reasoning mid-conversation while preserving cache:** Add a `configuration_update` input item to increase reasoning effort for difficult work or reduce it for routine follow-ups without rewriting the original prompt prefix. The updated reasoning effort applies until another `configuration_update` input item overrides it. See [Change reasoning mid-conversation](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation) for examples and compatibility.
- **Misalignment monitoring:** As part of our <a href="https://openai.com/index/path-to-astra/" target="_blank" rel="noopener noreferrer">strengthened safeguards</a> for GPT-6 Astra, our systems asynchronously monitor for misalignment and trigger alerts when necessary. See [Misalignment monitoring](https://developers.openai.com/api/docs/guides/safety-checks/misalignment-monitoring) for more information.
- **Limitations:** GPT-6 Astra does not support the `none` reasoning effort. [Fast mode](https://developers.openai.com/api/docs/guides/fast-mode) is unavailable for GPT-6 Astra with EU data residency.

GPT-6 Astra also supports the existing API capabilities available with GPT-5.6, including [computer use](https://developers.openai.com/api/docs/guides/tools-computer-use), [Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs), [streaming](https://developers.openai.com/api/docs/guides/streaming-responses), [Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling), [multi-agent orchestration](https://developers.openai.com/api/docs/guides/responses-multi-agent), [prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching), [persisted reasoning](https://developers.openai.com/api/docs/guides/reasoning#preserve-reasoning-across-calls), [compaction](https://developers.openai.com/api/docs/guides/compaction), and [pro mode](https://developers.openai.com/api/docs/guides/reasoning#reasoning-mode).

## Migration quickstart

### Migrate with Codex

Codex can apply the recommended changes in this guide with the <a href="https://github.com/openai/skills/tree/main/skills/.curated/openai-docs" target="_blank" rel="noopener noreferrer">OpenAI Docs skill</a>.

```text
$openai-docs migrate this project to GPT-6 Astra
```

To use this skill in other coding agents, download it from the <a href="https://github.com/openai/skills/tree/main/skills/.curated/openai-docs" target="_blank" rel="noopener noreferrer">OpenAI skills repository</a>.

### Update API and model parameters

Set `model` to `gpt-6-astra`, then check the following:

- **Reasoning effort:** If you currently use `none` or `minimal`, start with `low` and compare results. Otherwise, preserve your current effective [reasoning effort](https://developers.openai.com/api/docs/guides/reasoning#reasoning-effort). Use `reasoning.effort` in Responses or `reasoning_effort` in Chat Completions.
- **Tool calling:** Use the [Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses#migrating-from-chat-completions). GPT-6 Astra supports Chat Completions, but tool calling requires Responses.
- **Unsupported parameters:** Remove `temperature`, `top_p`, and `top_logprobs`. For Chat Completions, also remove `logprobs`. For Responses, remove `message.output_text.logprobs` from `include`.
- **Fast mode:** For EU data residency, use Standard processing. GPT-6 Astra does not support `service_tier: "fast"` or `service_tier: "priority"` with EU data residency. Fast mode for GPT-6 Astra does not include a latency SLA. See [Fast mode compatibility](https://developers.openai.com/api/docs/guides/fast-mode#is-fast-mode-compatible-with-data-residency-zero-data-retention-and-a-baa).
- **Changing reasoning effort:** If your application changes effort between responses, use `configuration_update` items in standard, single-agent requests. Keep request-level `reasoning.effort` unchanged to preserve the prompt prefix for caching. Check the [compatibility limits](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation) before adopting this feature.
- **Prompt caching:** When migrating from GPT-5.5 or earlier, replace `prompt_cache_retention` with `prompt_cache_options.ttl` set to `"30m"`. Review the [prompt caching changes](https://developers.openai.com/api/docs/guides/prompt-caching#summary-of-model-differences), including cache boundaries and cache-write billing.
- **Unnecessary approval pauses:** If you run into issues where the model keeps asking for approval before proceeding, use the [initiative and follow-through guidance](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#initiative-and-follow-through) to prompt for more autonomous execution. See the rest of [Prompting best practices](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices) for guidance on instruction following, writing style, subagent delegation, and testing.
