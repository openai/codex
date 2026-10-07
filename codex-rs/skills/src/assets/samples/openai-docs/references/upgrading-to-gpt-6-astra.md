# Upgrading to the GPT-6 model family

Use this guide when the user asks to migrate an existing OpenAI API integration, repository, prompt stack, agent, model router, or model picker to GPT-6 Astra, GPT-6.1 Sol, GPT-6 Sol, or GPT-6 Luna after verifying the target's availability.

Preserve an explicitly requested target. For an unspecified target, use the live model-selection or migration guidance rather than assuming every workload should use Astra.

Before changing code, retrieve the current live GPT-6 model guidance using already-callable official documentation search and fetch, or immediately use official-domain web search and fetch:

https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md

For prompt changes, also read only the `## Prompting best practices` section from:

https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices

Treat live docs as canonical for current model IDs, parameters, limits, pricing, and feature availability. The skill-specific workflow below covers repository inspection, scope preservation, and validation. The fallback after it includes all non-prompting guidance, including access notices, examples, caveats, and `## Migration quickstart`; the full prompting section is in `references/prompting-guide.md`. When refreshing, preserve all guide content unless it is specific to the website rather than useful to the skill, and record any omission. Remove website metadata and component markup while retaining their readable content. Resolve site-relative links against `https://developers.openai.com`. Keep section-only links as local fragments when their intended target exists in the same reference file; use canonical absolute URLs for cross-document or absent targets and explicit fetch/source instructions.

Verify the exact model, endpoint, service tier, and region; if support is unverified, report the unresolved requirement rather than infer support.

## Core principle

Do not perform a blind model-string replacement.

First preserve the behavior, latency class, cost class, reasoning level, endpoint contract, tool semantics, cache behavior, and output contract of each usage site. Then make the smallest safe migration. Adopt new GPT-6 capabilities only when they solve a measured problem or the user explicitly asks for them.

Set a supported reasoning effort explicitly to preserve the source model's effective behavior; verify omitted defaults rather than guessing.

## Migration posture

Classify every usage site before editing:

1. `simple Astra migration`
   - One flagship model usage.
   - Same endpoint and request shape can remain.
   - Reasoning effort is explicit or its old effective value is known.
   - No cache, vision, file, tool, or parser behavior needs implementation changes.
2. `tier-aware family migration`
   - The repository exposes multiple model roles, model choices, fallbacks, routers, pricing data, or capability metadata.
   - Map each role to a verified suitable target instead of replacing everything with Astra.
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

## Structured outputs, parsers, and tool contracts

Keep output contracts explicit:

- preserve JSON schemas, required fields, enums, refusal handling, and parser expectations;
- preserve tool names, parameter schemas, call IDs, and retry behavior;
- keep citations, evidence fields, or native artifacts when downstream consumers require them;
- validate that the final answer still satisfies the contract, not merely that a tool call succeeded.

Do not fix a failing migration by weakening a schema, deleting required behavior, removing routes, dropping tools, or changing business logic unless the user explicitly asked for that product change.

## Prompt migration judgment

After the model and API baseline is working, run representative traces before editing prompts. Change prompts for measured failures, documented migration requirements, or an explicit user request. Read `references/prompting-guide.md` for the exact canonical prompting section when prompt changes are needed.

## Upgrade workflow

1. Fetch current live GPT-6 docs. Fetch the Prompting Best Practices section only when prompt changes are needed.
2. Inventory every usage site and its adjacent prompt, config, registry, parser, and test surfaces.
3. Classify each usage by role and migration class.
4. Choose the verified available target by the existing workload's role. Do not infer model availability merely because it appears in this fallback.
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
3. GPT-6 target + the smallest prompt or API fix required by a measured failure;
4. optional feature treatment, isolated from the baseline.

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

For model routers and pickers, test at least one representative workload for each intended role against its quality, latency, and cost requirements.

## Required final report

Return:

- `Current usage inventory`: each model site, endpoint, role, prompt surface, and old effective reasoning.
- `Target mapping`: the exact requested or resolved model, unchanged, or confirmation-needed, with the reason and any availability or compatibility blocker.
- `Changes made`: model strings, reasoning settings, prompts, registries, metadata, tests, and API-shape changes.
- `Compatibility checks`: Chat Completions/tools, caching, state replay, multimodal detail, context/cost, schemas, and mixed-model routing.
- `Prompt changes`: each surgical edit and the failure mode it addresses.
- `Validation`: commands, evals, traces, before/after measurements, and remaining gaps.
- `Unchanged sites`: historical, pinned, ambiguous, or intentionally role-specific usages.
- `Blockers and open questions`: exact issue, why it is unsafe to guess, and the smallest next step.

Never say the migration is complete merely because model strings changed. It is complete only when the affected behavior and contracts have been validated or the remaining gaps are stated explicitly.

Choose a GPT-6 model based on the reasoning your task requires, speed, and cost.

- [GPT-6 Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)

  **Highest intelligence**

  For the most demanding reasoning, coding, and professional work.

- [GPT-6.1 Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol)

  **Balanced speed, cost, and intelligence**

  Near-Astra performance for complex work at a lower cost.

- [GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna)

  **Fastest and most cost-effective**

  Strong performance for focused, high-volume tasks.

To get started, set `model` in a [Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses) request. If you already use [`gpt-6-sol`](https://developers.openai.com/api/docs/models/gpt-6-sol), review the [migration guidance](#migration-quickstart) before switching to GPT-6.1 Sol.

### GPT-6 Astra

GPT-6 Astra is our most intelligent model yet, with state-of-the-art performance in computer use, browsing, software engineering, science, and professional work. It can carry out multi-step workflows across code, browsers, and professional software. In [several evaluations](https://openai.com/index/gpt-6-astra/), Astra achieved stronger results using substantially fewer output tokens. Its estimated API cost per task was lower than earlier models despite its higher per-token pricing.

GPT-6 Astra is also our most aligned model yet. It excels at exercising care, respecting task boundaries, and communicating transparently. When instructions leave room for interpretation, it uses the context it has to fill in routine gaps and asks focused questions when the answer could change the outcome. It incorporates new requirements, changes course when asked, and answers side questions without losing track of the broader task.

All GPT-6 Astra users also have access to [Fast mode](https://developers.openai.com/api/docs/guides/fast-mode) and the new [Ultrafast mode](https://developers.openai.com/api/docs/guides/ultrafast-mode) for our fastest API speeds.

### GPT-6.1 Sol

Use GPT-6.1 Sol for complex coding, computer use, and professional work when you
want near-Astra performance at a lower cost. Compare it with Astra on your tasks
to assess the tradeoff between quality and cost.

Set `reasoning.effort` to `low`, `medium` (default), `high`, `xhigh`, or `max`.
Use the Responses API for tool calling. Chat Completions supports requests
without tools. The `none` and `minimal` reasoning efforts are not supported.

See the [model page](https://developers.openai.com/api/docs/models/gpt-6.1-sol) for specifications,
pricing, and availability, or [model selection](https://developers.openai.com/api/docs/guides/model-selection#when-to-consider-gpt-61-sol)
for guidance on choosing a model.

## What's new

- **Async tool calling:** GPT-6 can continue reasoning, call other tools, or answer independent parts of a request while your application runs a tool. Set `async: true` on a function or custom tool and return its result when ready using the original `call_id`. Your application still executes the tool and manages pending work. See [Async tool calling](https://developers.openai.com/api/docs/guides/async-tool-calling) for basic usage and a developer-defined wait-tool pattern.
- **Mid-turn steering:** Send additional user instructions while GPT-6 is working, such as a correction or a change in requirements. Over a WebSocket connection, the Responses API preserves completed work and includes the update in a continuation. See [Mid-turn steering](https://developers.openai.com/api/docs/guides/steering) for the event flow and tool-result handling.
- **Change reasoning mid-conversation while preserving cache:** Add a `configuration_update` input item to increase reasoning effort for difficult work or reduce it for routine follow-ups without rewriting the original prompt prefix. The updated reasoning effort applies until another `configuration_update` input item overrides it. See [Change reasoning mid-conversation](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation) for examples and compatibility.
- **Misalignment monitoring:** As part of our [strengthened safeguards](https://openai.com/index/path-to-astra/) for GPT-6 Astra, our systems asynchronously monitor for misalignment and trigger alerts when necessary. See [Misalignment monitoring](https://developers.openai.com/api/docs/guides/safety-checks/misalignment-monitoring) for more information.

GPT-6 also supports the existing API capabilities available with GPT-5.6, including [computer use](https://developers.openai.com/api/docs/guides/tools-computer-use), [Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs), [streaming](https://developers.openai.com/api/docs/guides/streaming-responses), [Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling), [multi-agent orchestration](https://developers.openai.com/api/docs/guides/responses-multi-agent), [prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching), [persisted reasoning](https://developers.openai.com/api/docs/guides/reasoning#preserve-reasoning-across-calls), [compaction](https://developers.openai.com/api/docs/guides/compaction), and [pro mode](https://developers.openai.com/api/docs/guides/reasoning#reasoning-mode).

## Limitations

- GPT-6 Astra and GPT-6.1 Sol do not support the `none` reasoning effort; GPT-6 Sol and GPT-6 Luna do.
- Fast mode is not available with EU data residency for GPT-6 Astra, GPT-6 Sol, or GPT-6 Luna. [Ultrafast mode](https://developers.openai.com/api/docs/guides/ultrafast-mode) supports US data residency and global processing only. It does not support EU or other non-US regional processing endpoints. See [data residency eligibility](https://developers.openai.com/api/docs/guides/your-data#which-models-and-features-are-eligible-for-data-residency).

## Migration quickstart

### Migrate with Codex

Codex can apply the recommended changes in this guide with the [OpenAI Docs skill](https://github.com/openai/codex/tree/main/codex-rs/skills/src/assets/samples/openai-docs).

```text
$openai-docs migrate this project to the GPT-6 model family
```

To use this skill in other coding agents, download it from the [Codex repository](https://github.com/openai/codex/tree/main/codex-rs/skills/src/assets/samples/openai-docs).

### Update API and model parameters

Set `model` to `gpt-6-astra`, `gpt-6.1-sol`, or `gpt-6-luna`, then check the following:

- **Reasoning effort:** Preserve your current effective [reasoning effort](https://developers.openai.com/api/docs/guides/reasoning#reasoning-effort) where supported. GPT-6 Astra and GPT-6.1 Sol do not support `none`; use `low` instead. GPT-6 Sol and GPT-6 Luna support `none`. If your existing request uses `minimal`, start with `low` and compare results on representative tasks. Use `reasoning.effort` in Responses or `reasoning_effort` in Chat Completions.
- **Tool calling:** Use the [Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses#migrating-from-chat-completions). GPT-6 Astra and GPT-6.1 Sol support Chat Completions, but tool calling requires Responses. GPT-6 Sol and GPT-6 Luna support function calling in Chat Completions only with `reasoning_effort: "none"`. Use Responses for reasoning with tools.
- **Unsupported parameters:** When reasoning effort is not `none`, remove `temperature`, `top_p`, and `top_logprobs`. For Chat Completions, also remove `logprobs`. For Responses, remove `message.output_text.logprobs` from `include`.
- **Data residency:** Fast mode is not available with EU data residency for GPT-6 Astra, GPT-6 Sol, or GPT-6 Luna. [Ultrafast mode](https://developers.openai.com/api/docs/guides/ultrafast-mode) supports US data residency and global processing only. It does not support EU or other non-US regional processing endpoints. Fast mode for GPT-6 Astra does not include a latency SLA. See [Fast mode compatibility](https://developers.openai.com/api/docs/guides/fast-mode#is-fast-mode-compatible-with-data-residency-zero-data-retention-and-a-baa).
- **Changing reasoning effort:** If your application changes effort between responses, use `configuration_update` items in standard, single-agent requests. Keep request-level `reasoning.effort` unchanged to preserve the prompt prefix for caching. Check the [compatibility limits](https://developers.openai.com/api/docs/guides/reasoning#change-reasoning-mid-conversation) before adopting this feature.
- **Prompt caching:** When migrating from GPT-5.5 or earlier, replace `prompt_cache_retention` with `prompt_cache_options.ttl` set to `"30m"`. Review the [prompt caching changes](https://developers.openai.com/api/docs/guides/prompt-caching#summary-of-model-differences), including cache boundaries and cache-write billing.
- **Unnecessary approval pauses:** If you run into issues where the model keeps asking for approval before proceeding, use the [initiative and follow-through guidance](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#initiative-and-follow-through) to prompt for more autonomous execution. See the rest of [Prompting best practices](https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices) for guidance on instruction following, writing style, subagent delegation, and testing.
