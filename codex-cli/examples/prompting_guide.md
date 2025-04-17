# Prompting guide

1. [Starter task](#starter-task)
2. [Custom instructions](#custom-instructions)
3. [Prompting techniques](#prompting-techniques)

## Starter task

To see how the Codex CLI works, run:

```
codex --help
```

You can also ask it directly:

```
codex "write 2-3 sentences on what you can do"
```

To get a feel for the mechanics, let's ask Codex to create a simple HTML webpage. In a new directory run:

```
mkdir first-task && cd first-task
git init
codex "Create a file poem.html that renders a poem about the nature of intelligence and programming by you, Codex. Add some nice CSS and make it look like it's framed on a wall"
```

By default, Codex will be in `suggest` mode. Select "Yes (y)" until it completes the task.

You should see something like:

```
poem.html has been added.

Highlights:
- Centered “picture frame” on a warm wall‑colored background using flexbox.
- Double‑border with drop‑shadow to suggest a wooden frame hanging on a wall.
- Poem is pre‑wrapped and nicely typeset with Georgia/serif fonts, includes title and small signature.
- Responsive tweaks keep the frame readable on small screens.

Open poem.html in a browser and you’ll see the poem elegantly framed on the wall.
```

Enter "q" to exit out of the current session and `open poem.html`. You should see a webpage with a custom poem!

## Custom instructions

Codex supports two types of Markdown-based instruction files that influence model behavior and prompting:

### `~/.codex/instructions.md`

Global, user-level custom guidance injected into every session. You should keep this relatively short and concise. These instructions are applied to all Codex runs across all projects and are great for personal defaults, shell setup tips, safety constraints, or preferred tools.

**Example:** "Before executing shell commands, create and activate a `.codex-venv` Python environment." or "Avoid running pytest until you've completed all your changes."

### `CODEX.md`

Project-specific instructions loaded from the current directory or Git root. Use this for repo-specific context, file structure, command policies, or project conventions. These are automatically detected unless `--no-project-doc` or `CODEX_DISABLE_PROJECT_DOC=1` is set.

**Example:** “All React components live in `src/components/`".

## Prompting techniques

We recently published a [GPT 4.1 prompting guide](https://cookbook.openai.com/examples/gpt4-1_prompting_guide) which contains excellent intuitions for getting the most out of our latest models. It also contains content for how to build agentic workflows from scratch, which may be useful when customizing the Codex CLI for your needs. The Codex CLI is a reference implementation for agentic coding, and puts into practice many of the ideas in that document.

There are three common prompting patterns when working with Codex. They roughly traverse task complexity and the level of agency you wish to provide to the Codex CLI.

### Small requests

For cases where you want Codex to make a minor code change, such as fixing a self-contained bug or adding a small feature, specificity is important. Try to identify the exact change in a way that another human could reflect on your task and verify if their work matches your requirements.

**Example:** From the directory above `/utils`:

`codex "Modify the discount function utils/priceUtils.js to apply a 10 percent discount"`

**Key principles**:

- Name the exact function or file being edited
- Describe what to change and what the new behavior should be
- Default to interactive mode for faster feedback loops

### Medium tasks

For more complex tasks requiring longer form input, you can write the instructions as a file on your local machine:

`codex "$(cat task_description.md)"`

We recommend putting a sufficient amount of detail that directly states the task in a short and simple description. Add any relevant context that you’d share with someone new to your codebase (if not already in `CODEX.md`). You can also include any files Codex should read for more context, edit or take inspiration from, along with any preferences for how Codex should verify its work.

If Codex doesn’t get it right on the first try, give feedback to fix when you're in interactive mode!

**Example**: content of `task_description.md`:

```
Refactor: simplify model names across static documentation

Can you update docs_site to use a better model naming convention on the site.

Read files like:
- docs_site/content/models.md
- docs_site/components/ModelCard.tsx
- docs_site/utils/modelList.ts
- docs_site/config/sidebar.ts

Replace confusing model identifiers with a simplified version wherever they’re user-facing.

Write what you changed or tried to do to final_output.md
```

### Large projects

Codex can be surprisingly self-sufficient for bigger tasks where your preference might be for the agent to do some heavy lifting up front, and allow you to refine its work later.

In such cases where you have a goal in mind but not the exact steps, you can structure your task to give Codex more autonomy to plan, execute and track its progress.

For example:

- Add a `.codex/` directory to your working directory. This can act as a shared workspace for you and the agent.
- Seed your project directory with a high-level requirements document containing your goals and instructions for how you want it to behave as it executes.
- Instruct it to update its plan as it progresses (i.e. "While you work on the project, create dated files such as `.codex/plan_2025-04-16.md` containing your planned milestones, and update these documents as you progress through the task. For significant pieces of completed work, update the `README.md` with a dated changelog of each functionality introduced and reference the relevant documentation.")

_Note: `.codex/` in your working directory is not special-cased by the CLI like the custom instructions listed above. This is just one recommendation for managing shared-state with the model. Codex will treat this like any other directory in your project._

### Modes of interaction

For each of these levels of complexity, you can control the degree of autonomy Codex has: let it run in full-auto and audit afterward, or stay in interactive mode and approve each milestone.
