## Custom Prompts

Save frequently used prompts as Markdown files and reuse them quickly from the slash menu.

- Location: Put files in `$CODEX_HOME/prompts/` (defaults to `~/.codex/prompts/`).
- File type: Only Markdown files with the `.md` extension are recognized.
- Name: The filename without the `.md` extension becomes the slash entry. For a file named `my-prompt.md`, type `/my-prompt`.
- Content: The file contents are sent as your message when you select the item in the slash popup and press Enter.
- How to use:
  - Start a new session (Codex loads custom prompts on session start).
  - In the composer, type `/` to open the slash popup and begin typing your prompt name.
  - Use Up/Down to select it. Press Enter to submit its contents, or Tab to autocomplete the name.
- Notes:
  - Files with names that collide with built‑in commands (e.g. `/init`) are ignored and won’t appear.
  - New or changed files are discovered on session start. If you add a new prompt while Codex is running, start a new session to pick it up.

### Transcript redaction of saved prompts

By default, when you submit a saved prompt via the slash popup, Codex sends the prompt file’s body to the model but shows only the typed command in the transcript (for example, `/my-prompt`). This keeps long or sensitive prompt bodies out of the visible chat log while still using their contents.

You can disable this redaction and show the saved prompt body instead:

- CLI flag (interactive TUI):
  - `--show-saved-prompt` (alias: `--no-redact-saved-prompt`)
- Config file (config.toml):
  - `redact_saved_prompt_body = false` (default is `true`)

When redaction is disabled, the transcript will display the full saved prompt body as the user message.
