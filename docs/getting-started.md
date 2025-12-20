## Getting started

Looking for something specific? Jump ahead:

- [Tips & shortcuts](#tips--shortcuts) – hotkeys, resume flow, prompts
- [Non-interactive runs](./exec.md) – automate with `codexel exec`
- Ready for deeper customization? Head to [`advanced.md`](./advanced.md)

### CLI usage

| Command              | Purpose                            | Example                           |
| -------------------- | ---------------------------------- | --------------------------------- |
| `codexel`            | Interactive TUI                    | `codexel`                         |
| `codexel "..."`      | Initial prompt for interactive TUI | `codexel "fix lint errors"`       |
| `codexel exec "..."` | Non-interactive "automation mode"  | `codexel exec "explain utils.ts"` |

Key flags: `--model/-m`, `--ask-for-approval/-a`.

### Resuming interactive sessions

- Run `codexel resume` to display the session picker UI
- Resume most recent: `codexel resume --last`
- Resume by id: `codexel resume <SESSION_ID>` (You can get session ids from /status or `~/.codexel/sessions/`)
- The picker shows the session's recorded Git branch when available.
- To show the session's original working directory (CWD), run `codexel resume --all` (this also disables cwd filtering and adds a `CWD` column).

Examples:

```shell
# Open a picker of recent sessions
codexel resume

# Resume the most recent session
codexel resume --last

# Resume a specific session by id
codexel resume 7f9f9a2e-1b3c-4c7a-9b0e-123456789abc
```

### Running with a prompt as input

You can also run Codexel with a prompt as input:

```shell
codexel "explain this codebase to me"
```

### Example prompts

Below are a few bite-size examples you can copy-paste. Replace the text in quotes with your own task.

| ✨  | What you type                                                                     | What happens                                                               |
| --- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 1   | `codexel "Refactor the Dashboard component to React Hooks"`                       | Codexel rewrites the class component, runs `npm test`, and shows the diff. |
| 2   | `codexel "Generate SQL migrations for adding a users table"`                      | Infers your ORM, creates migration files, and runs them in a sandboxed DB. |
| 3   | `codexel "Write unit tests for utils/date.ts"`                                    | Generates tests, executes them, and iterates until they pass.              |
| 4   | `codexel "Bulk-rename *.jpeg -> *.jpg with git mv"`                               | Safely renames files and updates imports/usages.                           |
| 5   | `codexel "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`                      | Outputs a step-by-step human explanation.                                  |
| 6   | `codexel "Carefully review this repo, and propose 3 high impact well-scoped PRs"` | Suggests impactful PRs in the current codebase.                            |
| 7   | `codexel "Look for vulnerabilities and create a security review report"`          | Finds and explains security bugs.                                          |

Looking to reuse your own instructions? Create slash commands with [custom prompts](./prompts.md).

### Memory with AGENTS.md

You can give Codexel extra instructions and guidance using `AGENTS.md` files. Codexel looks for them in the following places, and merges them top-down:

1. `~/.codexel/AGENTS.md` - personal global guidance
2. Every directory from the repository root down to your current working directory (inclusive). In each directory, Codex first looks for `AGENTS.override.md` and uses it if present; otherwise it falls back to `AGENTS.md`. Use the override form when you want to replace inherited instructions for that directory.

For more information on how to use AGENTS.md, see the [official AGENTS.md documentation](https://agents.md/).

### Tips & shortcuts

#### Use `@` for file search

Typing `@` triggers a fuzzy-filename search over the workspace root. Use up/down to select among the results and Tab or Enter to replace the `@` with the selected path. You can use Esc to cancel the search.

#### Answer interactive questions

When Codexel needs a decision mid-run, it may pause and show an interactive question picker instead of continuing.

- Use arrow keys to move, Enter to choose/confirm, and Esc to cancel.
- Some questions support multi-select (Space toggles selections).
- A free-text option is always available for custom input (you do not need to type it as an explicit option).

#### Plan with `/plan`

Use `/plan` to create a plan and approve it before making changes.

When you approve a plan, Codexel writes a Markdown copy to `.codexel/plan.md` under the session's working directory. The `.codexel/` directory is treated as project-internal and is hidden from the agent's built-in file tools (and `@` file search).

Tip: add `.codexel/` to your project's `.gitignore` if you don't want it committed.

#### Esc—Esc to edit a previous message

When the chat composer is empty, press Esc to prime “backtrack” mode. Press Esc again to open a transcript preview highlighting the last user message; press Esc repeatedly to step to older user messages. Press Enter to confirm and Codex will fork the conversation from that point, trim the visible transcript accordingly, and pre‑fill the composer with the selected user message so you can edit and resubmit it.

In the transcript preview, the footer shows an `Esc edit prev` hint while editing is active.

#### `--cd`/`-C` flag

Sometimes it is not convenient to `cd` to the directory you want Codexel to use as the "working root" before running Codexel. Fortunately, `codexel` supports a `--cd` option so you can specify whatever folder you want. You can confirm that Codexel is honoring `--cd` by double-checking the **workdir** it reports in the TUI at the start of a new session.

#### `--add-dir` flag

Need to work across multiple projects in one run? Pass `--add-dir` one or more times to expose extra directories as writable roots for the current session while keeping the main working directory unchanged. For example:

```shell
codexel --cd apps/frontend --add-dir ../backend --add-dir ../shared
```

Codex can then inspect and edit files in each listed directory without leaving the primary workspace.

#### Shell completions

Generate shell completion scripts via:

```shell
codexel completion bash
codexel completion zsh
codexel completion fish
```

#### Image input

Paste images directly into the composer (Ctrl+V / Cmd+V) to attach them to your prompt. You can also attach files via the CLI using `-i/--image` (comma‑separated):

```bash
codexel -i screenshot.png "Explain this error"
codexel --image img1.png,img2.jpg "Summarize these diagrams"
```

#### Environment variables and executables

Make sure your environment is already set up before launching Codexel so it does not spend tokens probing what to activate. For example, source your Python virtualenv (or other language runtimes), start any required daemons, and export the env vars you expect to use ahead of time.
