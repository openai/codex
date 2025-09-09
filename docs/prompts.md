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

### Custom instructions for saved prompts

You can append a custom instruction after the saved prompt name when sending it. This lets you specialize a reusable prompt at submission time.

- How to use:
  - Type the saved prompt name, then a space, then your instruction. Example:
    - `/my-prompt Please focus on performance trade‑offs`
  - Multiline instructions are supported. Example:
    - `/my-prompt\nSummarize key steps as a checklist.\nKeep answers concise.`

- What you’ll see:
  - The transcript shows exactly what you typed: `/my-prompt <your instruction>` (including newlines).
  - If redaction is enabled (default), the saved prompt body is still hidden in the transcript. You will see only the command and your instruction.

- What Codex sends to the model:
  - Codex wraps both your custom instruction and the saved prompt body to make your instruction high priority. The model receives a message that includes your instruction and then the saved prompt content.

- Turning redaction off:
  - CLI: pass `--show-saved-prompt` (alias: `--no-redact-saved-prompt`).
  - Config: set `redact_saved_prompt_body = false`.
