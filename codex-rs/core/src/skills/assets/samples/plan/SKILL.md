---
name: plan
description: Plan lifecycle management for Codex plans stored in $CODEX_HOME/plans (default ~/.codex/plans). Use when a user asks to create, find, read, update, delete, or manage plan documents for implementation work or overview/reference documentation.
---

# Plan

## Overview

Create and manage plan documents on disk. Plans stored on disk are markdown files with YAML frontmatter and free-form content. When drafting in chat, output only the plan body without frontmatter; add frontmatter only when stashing to disk. Support both implementation plans and overview/reference plans. Only write to the plans folder; do not modify the repository codebase.

## Core rules

- Resolve the plans directory as `$CODEX_HOME/plans` or `~/.codex/plans` when `CODEX_HOME` is not set.
- Create the plans directory if it does not exist.
- Never write to the repo; only read files to understand context.
- Require frontmatter with **only** `name` and `description` (single-line values) for on-disk plans.
- When presenting a draft plan in chat, omit frontmatter and start at `# Plan`.
- Enforce naming rules: short, lower-case, hyphen-delimited; filename must equal `<name>.md`.
- If a plan is not found, state it clearly and offer to create one.
- Allow overview-style plans that document flows, architecture, or context without a work checklist.

## Decide the task

1. **Find/list**: discover plans by frontmatter summary; confirm if multiple matches exist.
2. **Read/use**: validate frontmatter; present summary and full contents.
3. **Create**: inspect repo read-only; choose plan style (implementation vs overview); draft plan; write to plans directory only.
4. **Update**: load plan; revise content and/or description; preserve frontmatter keys; overwrite the plan file.
5. **Delete**: confirm intent, then remove the plan file if asked.

## Plan discovery

- Prefer `scripts/list_plans.py` for quick summaries.
- Use `scripts/read_plan_frontmatter.py` to validate a specific plan.
- If name mismatches filename or frontmatter is missing fields, call it out and ask whether to fix.

## Plan creation workflow

1. Read relevant docs and entry points (`README.md`, `docs/`, key modules) to scope requirements.
2. Identify scope, constraints, and data model/API implications (or capture existing behavior for an overview).
3. Draft either an ordered implementation plan or a structured overview plan with diagrams/notes as needed.
4. Immediately output the plan body only (no frontmatter), then ask the user if they want to 1. Make changes, 2. Implement it, 3. Stash it as per plan.
5. If the user wants to stash it, prepend frontmatter and save the plan under the computed plans directory using `scripts/create_plan.py`.

## Plan update workflow

- Re-read the plan and related code/docs before updating.
- Keep the plan name stable unless the user explicitly wants a rename.
- If renaming, update both frontmatter `name` and filename together.

## Scripts (low-freedom helpers)

Create a plan file (body only; frontmatter is written for you). Run from the plan skill directory:

```bash
python ./scripts/create_plan.py \
  --name codex-rate-limit-overview \
  --description "Scope and update plan for Codex rate limiting" \
  --body-file /tmp/plan-body.md
```

Read frontmatter summary for a plan (run from the plan skill directory):

```bash
python ./scripts/read_plan_frontmatter.py ~/.codex/plans/codex-rate-limit-overview.md
```

List plan summaries (optional filter; run from the plan skill directory):

```bash
python ./scripts/list_plans.py --query "rate limit"
```

## Plan file format

Use one of the structures below for the plan body. When drafting, output only the body (no frontmatter). When stashing, prepend this frontmatter:

```markdown
---
name: <plan-name>
description: <1-line summary>
---
```

### Implementation plan body template

```markdown
# Plan

<1-3 sentences: intent, scope, and approach.>

## Requirements
- <Requirement 1>
- <Requirement 2>

## Scope
- In:
- Out:

## Files and entry points
- <File/module/entry point 1>
- <File/module/entry point 2>

## Data model / API changes
- <If applicable, describe schema or contract changes>

## Action items
[ ] <Step 1>
[ ] <Step 2>
[ ] <Step 3>
[ ] <Step 4>
[ ] <Step 5>
[ ] <Step 6>

## Testing and validation
- <Tests, commands, or validation steps>

## Risks and edge cases
- <Risk 1>
- <Risk 2>

## Open questions
- <Question 1>
- <Question 2>
```

### Overview plan body template

```markdown
# Plan

<1-3 sentences: intent and scope of the overview.>

## Overview
<Describe the system, flow, or architecture at a high level.>

## Diagrams
<Include text or Mermaid diagrams if helpful.>

## Key file references
- <File/module/entry point 1>
- <File/module/entry point 2>

## Auth / routing / behavior notes
- <Capture relevant differences (e.g., auth modes, routing paths).>

## Current status
- <What is live today vs pending work, if known.>

## Action items
- None (overview only).

## Testing and validation
- None (overview only).

## Risks and edge cases
- None (overview only).

## Open questions
- None.
```

## Writing guidance

- Keep action items ordered and concrete; include file/entry-point hints.
- For overview plans, keep action items minimal and set sections to "None" when not applicable.
- Always include testing/validation and risks/edge cases in implementation plans.
- Use open questions only when necessary (max 3).
- If a section is not applicable, note "None" briefly rather than removing it.
