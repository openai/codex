# Slash Commands (TUI)

Codex TUI supports a set of built‑in slash commands you can trigger from the composer by typing `/`.

Below is a reference for built‑in commands. The popup lists these first, then any user prompts (saved Markdown prompts) that don’t collide with built‑ins.

| Command        | Description                                         | Notes |
| -------------- | --------------------------------------------------- | ----- |
| `/model`       | choose what model and reasoning effort to use       | Opens a model selection popup; persists choice when applicable. |
| `/approvals`   | choose what Codex can do without approval           | Opens approvals/sandbox policy popup. |
| `/review`      | review my changes and find issues                   | Opens review UI with presets and sources. |
| `/new`         | start a new chat during a conversation              | Clears current thread; starts a new session. |
| `/resume`      | resume a previous session (opens picker)            | See variants below for “last” and “by id”. |
| `/init`        | create an AGENTS.md file with instructions for Codex | Sends a starter prompt to create AGENTS.md. |
| `/compact`     | summarize conversation to prevent hitting the context limit | Asks the agent to compact context. |
| `/diff`        | show git diff (including untracked files)           | Renders a unified diff overlay; requires a git repo. |
| `/mention`     | mention a file                                      | Inserts `@` in the composer; type to fuzzy‑search files. |
| `/status`      | show current session configuration and token usage  | Prints model, cwd, approvals, usage, etc. |
| `/limits`      | visualize weekly and hourly rate limits             | Shows current rate limit status. |
| `/mcp`         | list configured MCP tools                           | Lists MCP servers from config. |
| `/logout`      | log out of Codex                                    | Removes credentials and exits the TUI. |
| `/quit`        | exit Codex                                          | Exits the TUI immediately. |

## /resume — Resume a previous session

Resume lets you continue an existing conversation and append to the same rollout (`~/.codex/sessions/**/rollout-*.jsonl`). It mirrors the functionality of the CLI `codex resume` subcommand.

- `/resume` — Open the session picker (search + pagination) and choose a session to resume.
- `/resume last` — Resume the most recent recorded session without showing the picker (equivalent to `codex resume --last`).
- `/resume <SESSION_ID>` — Resume a specific session by its UUID (equivalent to `codex resume <SESSION_ID>`).

Notes:
- The Resume picker shows the first user message as a preview, relative age, and absolute path. Use Up/Down to move, Enter to select, Esc to start a new session, and Ctrl‑C to cancel.
- During a running task, `/resume` is disabled to avoid interrupting work in progress. Wait for the task to complete first (same behavior as `/new`).
- When resuming, Codex continues writing to the same rollout file and preserves the original session metadata.

Tips:
- Tab in the slash popup completes to the selected command (e.g. typing `/re` then Tab completes to `/review ` or `/resume ` depending on selection).
- Custom prompts appear after built‑ins. Prompts whose names collide with built‑ins (e.g. `init.md`) are ignored in the popup.

See also: CLI usage in `docs/getting-started.md` and advanced headless flows in `docs/advanced.md`.
