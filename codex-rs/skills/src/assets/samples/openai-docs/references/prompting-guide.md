## Retrieve the live GPT-5.6 prompting guidance

Use the OpenAI Docs MCP to fetch the live GPT-5.6 prompting guidance from:

https://developers.openai.com/api/docs/guides/model-guidance?model=gpt-5.6#prompting-best-practices

Read only the `## Prompting Best Practices` section, stopping at the next H2 heading. The URL anchor points to the section visually, but the Docs MCP may return the full page, so explicitly extract only that section.

Treat the live section as the canonical model-specific prompting guidance. Use the local guidance below only for skill-specific migration judgment: deciding what to preserve, remove, rewrite, or test when adapting an existing prompt stack to GPT-5.6.

## Skill-specific migration judgment

GPT-5.6 works best when prompts define the outcome, important constraints, available evidence, and completion bar, then leave room for the model to choose an efficient path. Compared with earlier GPT-5 models, many applications can use shorter prompts and smaller tool sets without losing quality.

Do not carry over every instruction from an older prompt stack. Legacy prompts often repeat rules, prescribe unnecessary steps, expose irrelevant tools, or include examples that no longer change behavior. With GPT-5.6, this can encourage extra exploration, repeated validation, and larger accumulated context.

Start with the smallest prompt and tool set that passes your evals. Add an instruction, example, or tool only when it fixes a measured failure mode.

## Simplify prompts first

When migrating an existing prompt, remove redundant scaffolding before adding new GPT-5.6-specific instructions.

Trim:

- repeated statements of the same rule;
- generic “be thorough,” “be concise,” or “think step by step” language;
- examples that do not change behavior;
- process instructions for behavior the model already performs reliably;
- tools and tool descriptions unrelated to the task.

Keep:

- the user-visible outcome;
- success criteria and stopping conditions;
- safety, business, evidence, and permission constraints;
- tool-routing rules when the correct route is not obvious;
- required output shape and validation requirements.

Review the remaining instructions for contradictions. GPT-5-class models follow prompt contracts closely, so conflicting rules can create more instability than missing detail.

## Outcome-first prompts and stopping conditions

Describe the destination rather than prescribing every step. GPT-5.6 can usually choose an efficient search, tool, or reasoning path when the prompt states what good looks like.

Prefer:

    Resolve the customer's issue end to end.

    Success means:
    - make the eligibility decision from available policy and account evidence
    - complete any allowed action before responding
    - return completed_actions, customer_message, and blockers
    - if required evidence is missing, ask for the smallest missing field

Avoid unnecessary absolute rules. Use ALWAYS, NEVER, must, and only for true invariants such as safety rules, required fields, or actions that should never happen. For judgment calls, such as when to search, ask, use a tool, or keep iterating, prefer decision rules.

Preserve explicit user values. When the correct value is implicit, provide decision criteria and let the model reason from context or schema. Avoid universal defaults, keyword maps, and broad semantic shortcuts.

Add stopping conditions:

    Resolve the request in the fewest useful tool loops, but do not let loop
    minimization outrank correctness, required evidence, calculations, or
    required citations.

    After each result, ask whether the core request can now be answered with
    useful evidence. If yes, answer. If required evidence is still missing,
    name the missing fact and use the smallest useful fallback.

## Personality, collaboration, and response length

GPT-5.6 is efficient, direct, and more compressed than recent models. For customer-facing assistants and collaborative products, define both personality and collaboration style.

- Personality controls tone, warmth, directness, formality, humor, empathy, and polish.
- Collaboration style controls when the model asks questions, makes assumptions, takes initiative, explains tradeoffs, checks work, and handles uncertainty.

Keep both short. Personality should shape the user experience; collaboration instructions should shape task behavior. Neither should replace clear goals, success criteria, tool rules, or stopping conditions.

Use concrete writing controls:

    Lead with the conclusion. Include the evidence needed to support it, any
    material caveat, and the next action. Keep all required facts, decisions,
    caveats, and next steps. Trim introductions, repetition, generic reassurance,
    and optional background first.

Avoid generic “be brief,” “keep it short,” or “use minimal text” instructions. GPT-5.6 is already biased toward compression, and generic brevity can make it omit required evidence or parts of an artifact.

For customer-facing tone, prefer concrete guidance:

    Be direct and tactful. Acknowledge friction specifically when relevant.
    Avoid canned reassurance and unnecessary sign-offs.

Avoid blanket language rules such as “always respond in the user's language” unless that is truly the product requirement. Specify the intended output language and when it should change.

For editing, rewriting, summaries, and customer-facing drafts, tell the model what to preserve:

    Preserve the requested artifact, length, structure, genre, and factual claims
    first. Improve clarity, flow, and correctness without adding new claims,
    sections, or a more promotional tone unless requested.

## Autonomy and permissions

GPT-5.6 can be proactive and persistent. Define which level of action each request authorizes.

    For requests to answer, explain, review, diagnose, or plan, inspect the
    relevant materials and report the result. Do not implement changes unless
    the request also asks for them.

    For requests to change, build, or fix, make the requested in-scope local
    changes and run relevant non-destructive validation without asking first.

    Require confirmation for external writes, destructive actions, purchases,
    or a material expansion of scope.

Specify which local actions are safe without approval, such as reading files, inspecting logs, searching, editing in-scope code, and running non-destructive tests.

Avoid repeating “ask first” throughout the prompt. Repetition can cause unnecessary permission checks even for safe, expected actions.

For long-running work, define the current layer of work. Distinguish research, design, implementation, review, and external coordination so the model does not silently move from one layer to another.

## Tool routing

Expose only task-relevant tools. Tool descriptions should state what the tool does, when to use it, important return fields, and error behavior.

When correctness depends on prerequisite retrieval or lookup, say so:

    Before taking an action, resolve required discovery, retrieval, and
    validation steps. Do not skip a prerequisite because the intended final
    state seems obvious.

When several reads are independent, parallelize them. When one result determines the next action, keep the work sequential. After parallel retrieval, synthesize before acting.

If a tool returns empty, partial, or suspiciously narrow results, try one or two meaningful fallbacks before concluding that no result exists.

## Programmatic Tool Calling

Programmatic Tool Calling is useful when code can reduce large, structured intermediate results before they return to model context.

Use it for:

- filtering, joining, sorting, ranking, deduplication, and aggregation;
- batching across many similar records;
- repeated deterministic validation;
- large structured results that can be reduced to a compact schema.

Prefer direct tool calls when:

- one call is sufficient;
- intermediate outputs are already small;
- each result may change the next decision;
- an action requires approval;
- the final answer must preserve citations or native artifacts;
- the workflow requires semantic judgment between calls.

Do not rely on generic instructions such as “use Programmatic Tool Calling efficiently.” State the bounded stage, eligible tools, output schema, retry limit, stop condition, and handoff back to direct model judgment.

    Use Programmatic Tool Calling only for the bounded record-reduction stage.
    Call only the documented read-only tools. Filter and deduplicate the
    intermediate results, then emit exactly the required compact schema with
    evidence fields. Retry transient failures at most twice. Use direct tool
    calls for approval, semantic judgment, citations, and final validation.

Evaluate the final user-visible answer, not only the program result. Lower tokens, latency, calls, or turns are improvements only when the final answer still meets the required quality bar.

## Grounding, citations, and retrieval budgets

For grounded answers, citation behavior should be part of the prompt. Define what needs support, what counts as enough evidence, and how to behave when evidence is missing. Absence of evidence should not automatically become a factual “no.”

    For ordinary Q&A, start with one broad search using short, discriminative
    keywords. If the top results contain enough support for the core request,
    answer from those results.

    Make another retrieval call only when a required fact, owner, date, ID, or
    source is missing; the user asked for exhaustive coverage or comparison; a
    specific artifact must be read; or an important claim would otherwise be
    unsupported.

    Do not search again only to improve phrasing, add examples, or support
    nonessential detail.

For research and synthesis:

- cite only retrieved sources;
- attach citations to the claims they support;
- label inference separately from directly supported facts;
- state conflicts between sources;
- narrow the answer or report missing evidence instead of guessing.

For creative drafting, distinguish source-backed facts from creative wording. Do not invent names, metrics, dates, roadmap status, customer outcomes, or product capabilities to make a draft sound stronger.

## Long-running workflows and state

For multi-step or tool-heavy tasks, prompt for a short visible preamble before the first tool call, then sparse outcome-based updates at major phase changes. Do not ask the model to narrate routine tool calls.

    Before tool calls for a multi-step task, send a one- or two-sentence
    user-visible update that states the first step. During the task, update only
    when a major phase begins or a finding changes the plan. Each update should
    state one concrete outcome and the next step.

Preserve assistant phase values when replaying history so the model can distinguish commentary from the final answer. If using previous_response_id, prior assistant state is preserved automatically. If replaying history manually, preserve each original phase value unchanged.

Compact after major milestones rather than every turn. Keep the prompt functionally consistent after compaction and treat compacted items as opaque state.

Persisted reasoning is useful when the objective, assumptions, and priorities remain stable across turns. Use current-turn behavior when earlier reasoning is no longer relevant. Do not treat persisted reasoning as an always-on optimization: stale reasoning can add tokens, increase latency, and anchor the model to an outdated approach.

Prompt caching also affects prompt construction. Keep reusable prefixes stable and avoid unnecessary churn in large system prompts. Use explicit cache breakpoints only when they improve measured cache behavior and cost for the workload.

## Reasoning effort

Treat reasoning effort as a last-mile tuning knob, not the first response to a weak result.

- Preserve the current GPT-5.5 or GPT-5.4 reasoning effort as the baseline.
- Test the same setting and one level lower on representative tasks.
- Use low for latency-sensitive work when it preserves quality.
- Use medium as a balanced starting point.
- Use high or xhigh only when evals show a meaningful gain.
- Reserve max for the hardest quality-first workloads; do not recommend it globally.

Before increasing reasoning effort, check whether the prompt is missing a success criterion, dependency rule, tool-routing rule, or verification loop.

## Frontend and visual tasks

GPT-5.6 has stronger layout, visual hierarchy, and design judgment. Still provide product context, preserve the existing design system, and name the states and constraints that matter.

For incremental frontend changes:

- inspect and preserve existing design tokens, components, and patterns;
- do not add extra features or decorative UI unless requested;
- preserve responsive behavior and expected states;
- render and inspect the result before finalizing.

For vision, computer use, localization, or OCR tasks where spatial precision matters, choose image detail intentionally. Use original detail for large, dense, or coordinate-sensitive images when the extra input cost and latency are justified.

## Check work before finishing

Give GPT-5.6 access to tools that can validate the output, and state what validation matters.

For coding:

    After making changes, run the most relevant validation available:
    - targeted tests for changed behavior
    - type checks or lint checks when applicable
    - build checks for affected packages
    - a minimal smoke test when full validation is too expensive

    If validation cannot be run, explain why and describe the next best check.

For visual artifacts:

    Render the artifact before finalizing. Inspect layout, clipping, spacing,
    missing content, and visual consistency. Revise until the rendered output
    matches the requirements.

For implementation plans, include requirements, named resources or files, state transitions or data flow, validation checks, failure behavior, privacy or security considerations, and open questions that materially affect implementation.

## Suggested prompt structure

Use this structure as a starting point for complex prompts. Keep each section short. Add detail only where it changes behavior.

    Role: [the model's function and context]

    Personality: [tone and collaboration style]

    Goal: [user-visible outcome]

    Success criteria: [what must be true before the final answer]

    Constraints: [policy, safety, business, evidence, and side-effect limits]

    Tools: [which tools to use, when, and what not to use]

    Output: [sections, length, format, and tone]

    Stop rules: [when to retry, fallback, abstain, ask, or stop]

## Prompt migration workflow

When moving an existing application to GPT-5.6:

1. Switch the model and preserve the current reasoning effort.
2. Run representative evals before changing the prompt.
3. Remove obsolete scaffolding, repeated instructions, and irrelevant tools.
4. Add only the smallest targeted instruction that fixes a measured regression.
5. Re-run evals after each prompt or reasoning change.

Do not rewrite a working prompt stack all at once. Otherwise you cannot tell whether a behavior change came from the model, reasoning setting, prompt, tool set, or runtime.

When a prompt regresses, debug it with a small set of real traces. Identify the failure mode, find the instruction or contradiction that likely caused it, make a surgical edit, and rerun the same cases.
