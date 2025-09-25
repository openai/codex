## Custom Prompts

Save frequently used prompts as Markdown files and reuse them quickly from the slash menu.

- Location: Place files in either `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`) or `<project>/.codex/commands/` at the repository root. Project prompts are detected from the nearest git root; if no git repository is present, Codex falls back to the current working directory.
- File type: Only top-level Markdown files with the `.md` extension are recognized. Files in subdirectories are ignored.
- Name: The filename without the `.md` extension becomes the slash entry. For `my-prompt.md`, type `/my-prompt`.
- Content: When you select the item in the slash popup and press Enter, the file contents are sent as your message. Positional placeholders `${1}`, `${2}`, … are replaced with the space-separated arguments you type after the command (e.g. `/my-prompt foo bar` substitutes `${1}` with `foo` and `${2}` with `bar`). Unused placeholders stay unchanged.
- How to use:
  - Start a new session (Codex loads custom prompts on session start).
  - In the composer, type `/` to open the slash popup and begin typing your prompt name.
  - Use Up/Down to select it. Press Enter to submit its contents, or Tab to autocomplete the name.
- Notes:
  - Files whose names collide with built‑in commands (e.g. `/init`) are ignored and won’t appear.
  - If duplicate names exist across locations, Codex keeps the first copy and prints a warning at startup indicating which file was skipped.
  - New or changed files are discovered on session start. Restart Codex to pick up additions.
