## Custom Prompts

Save frequently used prompts as Markdown files and reuse them quickly from the slash menu.

- Location: Put files in `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`).
- File type: Only Markdown files with the `.md` extension are recognized.
- Name: The filename without the `.md` extension becomes the slash entry. For a file named `my-prompt.md`, type `/my-prompt`.
- Content: The file contents are sent as your message when you select the item in the slash popup and press Enter.
- Arguments: Local prompts support placeholders in their content:
  - `$1..$9` expand to the first nine positional arguments typed after the slash name
  - `$ARGUMENTS` expands to all arguments joined by a single space
  - `$$` is preserved literally
  - Quoted args: Wrap a single argument in double quotes to include spaces, e.g. `/review "docs/My File.md"`.
  - File picker: While typing a slash command, type `@` to open the file picker and fuzzy‑search files under the current working directory. Selecting a file inserts its path at the cursor; if it contains spaces it is auto‑quoted.
- How to use:
  - Start a new session (Codex loads custom prompts on session start).
  - In the composer, type `/` to open the slash popup and begin typing your prompt name.
  - Use Up/Down to select it. Press Enter to submit its contents, or Tab to autocomplete the name.
- Notes:
  - Files with names that collide with built‑in commands (e.g. `/init`) are ignored and won’t appear.
  - New or changed files are discovered on session start. If you add a new prompt while Codex is running, start a new session to pick it up.

### Slash popup rendering

When you type `/`, the popup lists built‑in commands and your custom prompts. For custom prompts, the popup shows only:

- A five‑word excerpt from the first non‑empty line of the prompt file, rendered dim + italic.

Details:

- The excerpt strips simple Markdown markers (backticks, `*`, `_`, leading `#`) and any `$1..$9`/`$ARGUMENTS` placeholders before counting words. If the line is longer than five words, it ends with an ellipsis `…`.
- If frontmatter provides an `argument-hint`, it appears inline after the excerpt; otherwise only the excerpt is shown. Placeholders still expand when you submit the prompt.

Examples (illustrative):

- Prompt file `perf-investigation.md` starts with: `Profile the slow path in module $1` → popup shows: `/perf-investigation  Profile the slow path in module`
- Prompt file `release-runbook.md` starts with: `Assemble release checklist for this service` → popup shows: `/release-runbook  Assemble release checklist`

Styling follows the Codex TUI conventions (command cyan + bold; excerpt dim + italic).

### Frontmatter (optional)

Prompt files may start with a YAML‑style block to describe how the command should appear in the palette. The frontmatter is stripped before the prompt body is sent to the model.

```
---
description: "Run a post-incident retro"
argument-hint: "[incident-id] [severity]"
---
Draft a post-incident retrospective for incident $1 (severity $2).
List the timeline, impacted subsystems, contributing factors, and next steps.
```

With this file saved as `incident-retro.md`, the popup row shows:
- Name: `/incident-retro`
- Description: `Run a post-incident retro`
- Argument hint: `[incident-id] [severity]`

### Argument examples

All arguments with `$ARGUMENTS`:

```
# search-codebase.md
Search the repository for $ARGUMENTS and summarize the files that need attention.
```

Usage: `/search-codebase async runtime contention` → `$ARGUMENTS` becomes `"async runtime contention"`.

Individual arguments with `$1`, `$2`, …:

```
# hotfix-plan.md
Prepare a hotfix plan for bug $1 targeting branch $2.
Assign engineering owners: $3.
Include smoke tests and rollback steps.
```

Usage: `/hotfix-plan BUG-1234 main "alice,bob"` → `$1` is `"BUG-1234"`, `$2` is `"main"`, `$3` is `"alice,bob"`.
