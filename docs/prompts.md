## Custom Prompts

Save frequently used prompts as Markdown files and reuse them quickly from the slash menu.

- Location: Put files in `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`).
- File type: Only Markdown files with the `.md` extension are recognized.
- Name: The filename without the `.md` extension becomes the slash entry. For a file named `my-prompt.md`, type `/my-prompt`.
- Content: The file contents are sent as your message when you select the item in the slash popup and press Enter.
- Argument substitution: Use `$0` (prompt name), `$1..$9` (positionals), and `$ARGUMENTS` (all args joined). Missing positionals remain literal. Works both when selecting from the popup and when typing `/name args` directly.
- Description in popup: Add a minimal front matter block at the top (`---` + `description: ...` + `---`); the block is stripped from the submitted body.
- History (custom prompts): Saved in history and reusable (previously not supported).
- How to use:
  - Start a new session (Codex loads custom prompts on session start).
  - In the composer, type `/` to open the slash popup and begin typing your prompt name.
  - Use Up/Down to select it. Press Enter to submit its contents, or Tab to autocomplete the name.
- Notes:
  - Files with names that collide with built‑in commands (e.g. `/init`) are ignored and won’t appear.
  - New or changed files are discovered on session start. If you add a new prompt while Codex is running, start a new session to pick it up.

 
