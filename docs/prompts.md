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
- Argument hints are intentionally omitted in the list to keep rows compact. Placeholders still expand when you submit the prompt.

Examples (illustrative):

- Prompt file `review.md` starts with: `Review this file carefully $1` → popup shows: `/review  Review this file carefully`
- Prompt file `changelog.md` starts with: `Summarize recent repo commits` → popup shows: `/changelog  Summarize recent repo commits`

Styling follows the Codex TUI conventions (command cyan + bold; excerpt dim + italic).

### Frontmatter (optional)

Prompt files may start with a YAML‑style block to describe how the command should appear in the palette. The frontmatter is stripped before the prompt body is sent to the model.

```
---
description: "Review a PR with context"
argument-hint: "[pr-number] [priority]"
---
Please review pull request #$1 with priority $2.
```

With this file saved as `review-pr.md`, the popup row shows:
- Name: `/review-pr`
- Description: `Review a PR with context`
- Argument hint: `[pr-number] [priority]`

### Argument examples

All arguments with `$ARGUMENTS`:

```
# review-all.md
Fix issue #$ARGUMENTS following our coding standards.
```

Usage: `/review-all 123 high-priority` → `$ARGUMENTS` becomes `"123 high-priority"`.

Individual arguments with `$1`, `$2`, …:

```
# review-pr.md
Review PR #$1 with priority $2 and assign to $3
```

Usage: `/review-pr 456 high alice` → `$1` is `"456"`, `$2` is `"high"`, `$3` is `"alice"`.
