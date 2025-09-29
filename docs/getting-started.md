## Getting started

### CLI usage

| Command            | Purpose                            | Example                         |
| ------------------ | ---------------------------------- | ------------------------------- |
| `codex`            | Interactive TUI                    | `codex`                         |
| `codex "..."`      | Initial prompt for interactive TUI | `codex "fix lint errors"`       |
| `codex exec "..."` | Non-interactive "automation mode"  | `codex exec "explain utils.ts"` |

Key flags: `--model/-m`, `--ask-for-approval/-a`.

### Resuming interactive sessions

- Run `codex resume` to display the session picker UI
- Resume most recent: `codex resume --last`
- Resume by id: `codex resume <SESSION_ID>` (You can get session ids from /status or `~/.codex/sessions/`)

Examples:

```shell
# Open a picker of recent sessions
codex resume

# Resume the most recent session
codex resume --last

# Resume a specific session by id
codex resume 7f9f9a2e-1b3c-4c7a-9b0e-123456789abc
```

### Managing multiple authentication profiles

If you log into Codex with more than one account, add `--auth-profile <NAME>`
to any `codex` invocation to isolate credentials and history per profile. The
CLI maps each profile to `~/.codex/profiles/<slug>`, where the slug is a
lowercase, hyphenated form of the name (for example, `"Work Account"`
becomes `work-account`).

Using Codex without the flag keeps the legacy behaviour: state is stored in
`~/.codex` and shared across all runs.

Examples:

```sh
# Launch the TUI with a personal profile (stored at ~/.codex/profiles/personal)
codex --auth-profile Personal

# Log in with a separate work account
codex --auth-profile "Work Account" login --api-key sk-...

# Run non-interactive commands inside that profile
codex --auth-profile "Work Account" exec "npm test"

# Resume a session saved under the same profile
codex --auth-profile "Work Account" resume --last
```

Exit messages now include the profile flag when one is active so you can
copy/paste the suggested command, for example:

```
To continue this session, run codex --auth-profile 'Work Account' resume <SESSION_ID>.
```

When `--auth-profile` is set, Codex exports two environment variables before
doing any work:

- `CODEX_HOME` points to the profile directory
- `CODEX_ACTIVE_AUTH_PROFILE` matches the name you passed on the command line

Both variables revert to their defaults when the flag is omitted.

### Running with a prompt as input

You can also run Codex CLI with a prompt as input:

```shell
codex "explain this codebase to me"
```

```shell
codex --full-auto "create the fanciest todo-list app"
```

That's it - Codex will scaffold a file, run it inside a sandbox, install any
missing dependencies, and show you the live result. Approve the changes and
they'll be committed to your working directory.

### Example prompts

Below are a few bite-size examples you can copy-paste. Replace the text in quotes with your own task.

| ✨  | What you type                                                                   | What happens                                                               |
| --- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 1   | `codex "Refactor the Dashboard component to React Hooks"`                       | Codex rewrites the class component, runs `npm test`, and shows the diff.   |
| 2   | `codex "Generate SQL migrations for adding a users table"`                      | Infers your ORM, creates migration files, and runs them in a sandboxed DB. |
| 3   | `codex "Write unit tests for utils/date.ts"`                                    | Generates tests, executes them, and iterates until they pass.              |
| 4   | `codex "Bulk-rename *.jpeg -> *.jpg with git mv"`                               | Safely renames files and updates imports/usages.                           |
| 5   | `codex "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`                      | Outputs a step-by-step human explanation.                                  |
| 6   | `codex "Carefully review this repo, and propose 3 high impact well-scoped PRs"` | Suggests impactful PRs in the current codebase.                            |
| 7   | `codex "Look for vulnerabilities and create a security review report"`          | Finds and explains security bugs.                                          |

### Memory with AGENTS.md

You can give Codex extra instructions and guidance using `AGENTS.md` files. Codex looks for `AGENTS.md` files in the following places, and merges them top-down:

1. `~/.codex/AGENTS.md` - personal global guidance
2. `AGENTS.md` at repo root - shared project notes
3. `AGENTS.md` in the current working directory - sub-folder/feature specifics

For more information on how to use AGENTS.md, see the [official AGENTS.md documentation](https://agents.md/).

### Tips & shortcuts

#### Use `@` for file search

Typing `@` triggers a fuzzy-filename search over the workspace root. Use up/down to select among the results and Tab or Enter to replace the `@` with the selected path. You can use Esc to cancel the search.

#### Image input

Paste images directly into the composer (Ctrl+V / Cmd+V) to attach them to your prompt. You can also attach files via the CLI using `-i/--image` (comma‑separated):

```bash
codex -i screenshot.png "Explain this error"
codex --image img1.png,img2.jpg "Summarize these diagrams"
```

#### Esc–Esc to edit a previous message

When the chat composer is empty, press Esc to prime “backtrack” mode. Press Esc again to open a transcript preview highlighting the last user message; press Esc repeatedly to step to older user messages. Press Enter to confirm and Codex will fork the conversation from that point, trim the visible transcript accordingly, and pre‑fill the composer with the selected user message so you can edit and resubmit it.

In the transcript preview, the footer shows an `Esc edit prev` hint while editing is active.

#### Shell completions

Generate shell completion scripts via:

```shell
codex completion bash
codex completion zsh
codex completion fish
```

#### `--cd`/`-C` flag

Sometimes it is not convenient to `cd` to the directory you want Codex to use as the "working root" before running Codex. Fortunately, `codex` supports a `--cd` option so you can specify whatever folder you want. You can confirm that Codex is honoring `--cd` by double-checking the **workdir** it reports in the TUI at the start of a new session.
