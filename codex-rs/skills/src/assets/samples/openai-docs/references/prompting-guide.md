## Retrieve the live GPT-5.6 prompting guidance

Use the OpenAI Docs MCP to fetch the live GPT-5.6 prompting guidance from:

https://developers.openai.com/api/docs/guides/model-guidance?model=gpt-5.6#prompting-best-practices

Read only the `## Prompting Best Practices` section, stopping at the next H2 heading. The URL anchor points to the section visually, but the Docs MCP may return the full page, so explicitly extract only that section.

Treat the live section as the canonical model-specific prompting guidance. Use the local guidance below only for skill-specific migration judgment: deciding what to preserve, remove, rewrite, or test when adapting an existing prompt stack to GPT-5.6.

## Skill-specific migration judgment

GPT-5.6 works best when prompts define the outcome, important constraints, available evidence, and completion bar, then leave room for the model to choose an efficient path.

Removing repeated instructions and examples and simplifying tool descriptions can improve task performance and token efficiency. In a sample of internal coding-agent eval runs, configurations with leaner system prompts improved evaluation scores by roughly 10–15% while reducing total tokens by 41–66% and cost by 33–67%. Results will vary by workload, so treat these ranges as directional and validate changes on representative tasks from your own application.

## Simplify prompts first

Start with a prompt and tool set that already works. Remove one group of instructions, examples, or tools at a time, then rerun the same evals.

Trim:

- repeated statements of the same rule;
- repeated style or process instructions that do not change behavior;
- examples that do not change behavior;
- process instructions for behavior the model already performs reliably;
- tools and tool descriptions unrelated to the task.

Keep:

- the user-visible outcome;
- success criteria and stopping conditions;
- safety, business, evidence, and permission constraints;
- tool-routing rules when the route depends on context;
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

GPT-5.6 tends to be more concise by default than GPT-5.5. When migrating, check whether broad brevity instructions such as “Be concise” or “Keep it short” are still useful. They may be unnecessary for some tasks and can sometimes make responses too brief. Keep them when they reliably produce the output your application needs.

For more consistent control across requests, use `text.verbosity` to set the default level of detail, then use the prompt for task-specific requirements. Choose `low`, `medium`, or `high` as the default level of detail for a request. In the prompt, specify any task-specific length, structure, or required content. See [Set up `text.verbosity`](https://developers.openai.com/api/docs/guides/deployment-checklist#set-up-textverbosity) for an API example.

For customer-facing assistants and collaborative products, define both personality and collaboration style.

- Personality controls tone, warmth, directness, formality, humor, empathy, and polish.
- Collaboration style controls when the model asks questions, makes assumptions, takes initiative, explains tradeoffs, checks work, and handles uncertainty.

Keep both short. Personality should shape the user experience; collaboration instructions should shape task behavior. Neither should replace clear goals, success criteria, tool rules, or stopping conditions.

When a task calls for a shorter answer, identify the information the model must preserve and the detail it can omit. For example:

    Lead with the conclusion. Include the evidence needed to support it, any material
    caveat, and the next action. Omit secondary detail and repetition.

    Keep all required facts, decisions, caveats, and next steps. Trim introductions,
    repetition, generic reassurance, and optional background first.

This gives the model a clear priority order: preserve the content needed to complete the task, then remove lower-value detail.

Broad labels such as “friendly” or “empathetic” can be ambiguous. Describe the writing choices that define your product's tone, such as how directly to state the answer, when to acknowledge a problem, and whether reassurance or a sign-off is appropriate.

    State the answer directly. If the user reports a problem, acknowledge the
    specific issue before giving the next step. Use reassurance only when it is
    relevant. Omit generic praise and unnecessary sign-offs.

Avoid blanket language rules such as “always respond in the user's language” unless that is truly the product requirement. Specify the intended output language and when it should change.

For editing, rewriting, summaries, and customer-facing drafts, tell the model what to preserve:

    Preserve the requested artifact, length, structure, genre, and factual claims
    first. Improve clarity, flow, and correctness without adding new claims,
    sections, or a more promotional tone unless requested.

## Define autonomy and approval boundaries

GPT-5.6 can be proactive and persistent when carrying out multi-step tasks. Define what level of action each request authorizes so the model can continue safe, in-scope work without unnecessary pauses while stopping before external, destructive, costly, or scope-expanding actions.

A compact policy is usually sufficient:

    For requests to answer, explain, review, diagnose, or plan, inspect the
    relevant materials and report the result. Do not implement changes unless
    the request also asks for them.

    For requests to change, build, or fix, make the requested in-scope local
    changes and run relevant non-destructive validation without asking first.

    Require confirmation for external writes, destructive actions, purchases,
    or a material expansion of scope.

Name safe local actions explicitly, such as reading files, inspecting logs, editing in-scope code, and running tests. Keep the policy in one place and state each rule once. Repeating instructions such as “ask first,” “do not mutate,” or “wait for approval” can cause unnecessary approval requests for safe, expected actions.

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

Programmatic Tool Calling (PTC) works best for bounded workflows where code can process several tool results or large intermediate outputs and return a much smaller structured result.

Multiple, parallel, or dependent calls alone do not justify Programmatic Tool Calling.

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

If both routes are needed, define one clear handoff and tell the model not to switch routes or repeat completed work.

The `program_output` item and final assistant `message` are separate outputs; make sure to test both. In theory, a program can return the correct records while the message omits a required field, citation, or caveat.

Compare direct and programmatic calling on the same representative tasks. Check whether the final response is correct, complete, and includes the required evidence. Then compare total tokens, latency, cost, calls, turns, and retries. Count lower resource use as an improvement only when the response still passes your existing evals.

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

Establish a baseline with the current reasoning effort before changing it.

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
