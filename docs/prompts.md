## Custom Prompts

Custom prompts turn your repeatable instructions into reusable slash commands, so you can trigger them without retyping or copy/pasting. Each prompt is a Markdown file that Codex expands into the conversation the moment you run it.

### Where prompts live

- **Location**: Codex loads prompts from two directories:
  - **Global**: `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`)
  - **Project-local**: `./.codex/prompts/` (relative to your current working directory)
- **Merging**: Prompts from both locations are combined. If the same prompt name exists in both directories, the project-local version takes precedence.
- **File type**: Codex only loads `.md` files. Non-Markdown files are ignored.
- **Naming**: The filename (without `.md`) becomes the prompt name. A file called `review.md` registers the prompt `review`.
- **Refresh**: Prompts are loaded when a session starts. Restart Codex (or start a new session) after adding or editing files.
- **Conflicts**: Files whose names collide with built-in commands (like `init`) are skipped.
- **Sorting**: The final prompt list is deduplicated and sorted alphabetically by name.

### File format

- Body: The file contents are sent verbatim when you run the prompt (after placeholder expansion).
- Frontmatter (optional): Add YAML-style metadata at the top of the file to improve the slash popup.

  ```markdown
  ---
  description: Request a concise git diff review
  argument-hint: FILE=<path> [FOCUS=<section>]
  ---
  ```

  - `description` shows under the entry in the popup.
  - `argument-hint` (or `argument_hint`) displays a short hint about expected inputs.

### Placeholders and arguments

- Numeric placeholders: `$1`–`$9` insert the first nine positional arguments you type after the command. `$ARGUMENTS` inserts all positional arguments joined by a single space. Use `$$` to emit a literal dollar sign (Codex leaves `$$` untouched).
- Named placeholders: Tokens such as `$FILE` or `$TICKET_ID` expand from `KEY=value` pairs you supply. Keys are case-sensitive—use the same uppercase name in the command (for example, `FILE=...`).
- Quoted arguments: Double-quote any value that contains spaces, e.g. `TICKET_TITLE="Fix logging"`.
- Invocation syntax: Run prompts via `/prompts:<name> ...`. When the slash popup is open, typing either `prompts:` or the bare prompt name will surface `/prompts:<name>` suggestions.
- Error handling: If a prompt contains named placeholders, Codex requires them all. You will see a validation message if any are missing or malformed.

### Running a prompt

1. Start a new Codex session (ensures the prompt list is fresh).
2. In the composer, type `/` to open the slash popup.
3. Type `prompts:` (or start typing the prompt name) and select it with ↑/↓.
4. Provide any required arguments, press Enter, and Codex sends the expanded content.

### Examples

**Draft PR helper**

`~/.codex/prompts/draftpr.md`

```markdown
---
description: Create feature branch, commit and open draft PR.
---

Create a branch named `tibo/<feature_name>`, commit the changes, and open a draft PR.
```

Usage: type `/prompts:draftpr` to have codex perform the work.
