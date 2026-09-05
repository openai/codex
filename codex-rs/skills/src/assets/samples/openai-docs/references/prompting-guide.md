## Retrieve the live GPT-6 prompting guidance

Use already-callable official documentation search and fetch, or immediately use official-domain web search and fetch, to retrieve the live GPT-6 prompting guidance from:

https://developers.openai.com/api/docs/guides/latest-model/gpt-6-astra.md#prompting-best-practices

Read only the `## Prompting best practices` section, stopping at the next H2 heading. The URL anchor points to the section visually, but a documentation fetch may return the full page, so explicitly extract only that section.

Treat the live section as the canonical model-specific prompting guidance. Use the local copy below only when live guidance is unavailable. Keep it identical to the page's `## Prompting best practices` section when refreshing this reference.

## Prompting best practices

GPT-6 Astra is more intelligent and capable than prior models like GPT-5.6 Sol, and also exhibits behavior patterns that can be optimized through prompting the model for your use case.

### GPT-6 Astra behavior

- [Initiative and follow-through](#initiative-and-follow-through) – The model is designed to be a more effective collaborator and is thus more likely to ask the user a question when additional input could materially change the result. This can cause it to stop when the user may expect it to make reasonable assumptions and persist.
- [Instruction following](#instruction-following) – GPT-6 Astra is stronger at general instruction following than our previous models, giving you greater control over its behavior. It can be more sensitive to instructions contained in skills and other files, such as `AGENTS.md`. We **strongly recommend** auditing skills and other files accessible to your model for instructions that could influence its behavior.
- [Personality and writing style](#personality-and-writing-style) – The model tends toward detailed, formatted responses and may use recurring phrases across sessions. Specify the writing style and structure your application needs.
- [Subagent delegation](#subagent-delegation) – The model may delegate less often than desired for your workflow. Specify when and how much it should use subagents for parallel work.
- [Testing and verification](#testing-and-verification) – For coding tasks, the model tends to be thorough in testing before considering a task complete. For smaller tasks, this can result in broader tests than the task requires.

### Initiative and follow-through

GPT-6 Astra is generally better than GPT-5.6 Sol and earlier models at staying coherent during long tasks. It is also more likely to ask for clarification where earlier models would make assumptions.

To encourage more autonomous work, start with this prompt:

```text
You should infer the user's intent and task scope from the instructions and prior conversation context. Your job is to bias towards action and carry the user's intended task to completion.

When the user expresses intent to perform new work or fix an existing issue, persist until the user's intended goal is complete. Progress autonomously towards the user's goal (e.g. creating isolated worktrees / checkouts if needed, resolving merge conflicts, read-only actions, creating draft PRs etc.) unless they are clearly destructive or irreversible.
```

When the user’s intent is unclear, the model is more likely to ask the user for clarification to proceed. Prompt the model to follow through if the user’s prompt implies authorization:

```text
When the user's prompt indicates a request for action, such as "can you...", "I want to...", "help me..." and similar expressions, treat these as instructions to do the work and take action. Do not stop at acknowledging capability (e.g. "Yes…"), proposing a plan, or offering to continue. Do not settle for a partial or "helpful enough" solution that does not fully satisfy the user's task to save time, effort or tokens. If a task requires sustained work, complete all the necessary work until the intended outcome is fulfilled.
```

Prompt the model to ask for approval only after preparing a concrete, reviewable result. This avoids blocking the task before the model has done the work it can, and often leads to quicker task completion.

```text
Before asking the user clarifying questions, you should complete the work that is already authorized from context and necessary to make the proposed action concrete and reviewable. The user should be approving a concrete, reviewable result. For example, before deploying a change, writing to an external application, merging a PR or publishing a site, do all the required work first so that user approval is the final step. You don't need user permission for reversible tasks, read-only actions, reviews or fixes, or anything for which authorization is provided earlier in the session or strongly implied from the task instruction.

Do not introduce unsolicited warnings, disclaimers, approval flows, or safety/compliance checklists due to hypothetical risk.
```

The model also likes to ask non-blocking questions as it’s working by default, so adjust these prompts to match the level of autonomy your application needs.

### Instruction following

GPT-6 Astra is better able to follow longer instructions, but can also be more sensitive to information in context. For example, unclear or conflicting guidance in a skill file may cause the model to pause and block work early. Make the priority of user instructions and skills explicit.

```text
The user's instructions take precedence over guidelines provided in a skill. If explicit user instructions conflict with a skill's instructions, prioritize the user's instructions.
```

Asking the model to identify the skill and instruction that caused it to pause or change direction can also be effective in providing transparency into model behavior.

```text
If a skill causes you to ask for permission or confirmation, pause, leave requested work unfinished, or diverge from the user's intent, name and link to the exact SKILL.md file you read, quote the relevant instruction, and briefly explain how it applies. Distinguish explicit skill requirements from your interpretation of guidelines.
```

Use this prompt to find silent and conflicting guidance when your application loads many skills and instruction files such as `AGENTS.md`.

### Personality and writing style

GPT-6 Astra tends to use lists, tables and Markdown to make responses scannable. If your application needs prose with less formatting, specify that preference.

```text
Default to using clear, concise paragraphs, each developing one main idea. Use lists only when the information is genuinely parallel, sequential, or easier to compare, and avoid nested lists unless the hierarchy cannot be expressed clearly in prose. Use plain, simple language: familiar words, concrete examples, and precise verbs. Prefer active voice and direct statements.

Make sure to state the main point clearly and early, then develop it with the explanation and detail the reader needs. Let each sentence build on what came before. Develop the points that matter and provide enough support to be useful.
```

For technical communication, the following prompt helps strike a balance between using clear, coherent language while remaining domain appropriate:

```text
Use plain language over jargon, and reference technical details only to the degree that it helps illustrate an idea or your work to the user. Communicate complex concepts in a clear and cohesive manner, and calibrate your writing to the level of background knowledge assumed from the user's prompt and context.
```

To reduce jargon and stock phrases in writing, start with this prompt:

```text
Avoid using slop words or phrases like "Bottom Line:" in conclusions, "delve," "foster," "leverage," "it's worth noting," "importantly," "Question? Answer." or "This isn't about X. It's about Y.", "genuinely" or hyphenated compound descriptions and adjectives. Do not use concluding summary statements such as "In short:..", "The simplest mental model is:...".

State the intended action directly. Avoid adding what you won't do, what will remain unchanged, or how you'll separate or categorize results. Do not use contrastive framing such as "X, not Y" or "X—not Y" that introduces an unprompted alternative that the user didn't ask about. Avoid invented compound labels like "exact-head checks" and "editorial-row layouts", vague qualifiers, and canned transitions; use plain verbs and prepositions to state the actual relationship directly.
```

### Subagent delegation

GPT-6 Astra is trained to be able to divide and delegate work to subagents that work in parallel. If you are implementing a multi-agent system in your harness, use the following prompt to tune how much GPT-6 Astra should delegate work:

```text
If at any point you can parallelize work by delegating tasks to another agent (no matter if you are the root or subagent), you should do so using collaboration tools if it could save time or improve quality.
```

Messages between agents may contain grammar or spacing errors. Use this prompt to make inter-agent messages easier to read:

```text
Messages that you send to other agents and your final answer may be read by a human, so ensure they are legible. Always put proper spaces between words and/or numbers.
```

The model tends to respond well to prompting for how and when it should delegate work to subagents, so tune this behavior to fit with your harness and multi-agent implementation.

### Testing and verification

For coding tasks, calibrate how much testing and verification a change requires. This can help avoid unnecessary tests or repeated checks for small changes.

```text
Do not write tests for reversible, low-impact changes that mirror the implementation. If you do choose to verify your work with tests, make sure that the tests are meaningful and necessary to verify implementation.

Run tests appropriate to the change and complete required checks. Once those pass, broaden or repeat testing only when new changes, failures, or unresolved concerns justify it; otherwise, continue toward completing the task.
```
