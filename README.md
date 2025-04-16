<h1 align="center">OpenAI Codex CLI</h1>
<p align="center">Lightweight coding agent that runs in your terminal</p>

<p align="center"><code>npm i -g @openai/codex</code></p>

![Codex demo GIF using: codex "explain this codebase to me"](./.github/demo.gif)

---

<details>
<summary><strong>Table&nbsp;of&nbsp;Contents</strong></summary>

- [Quickstart](#quickstart)
- [Why Codex?](#whycodex)
- [Security Model \& Permissions](#securitymodelpermissions)
  - [Platform sandboxing details](#platform-sandboxing-details)
- [System Requirements](#systemrequirements)
- [CLI Reference](#clireference)
- [Memory \& Project Docs](#memoryprojectdocs)
- [Non‑interactive / CI mode](#noninteractivecimode)
- [Recipes](#recipes)
- [Installation](#installation)
- [Configuration](#configuration)
- [FAQ](#faq)
- [Contributing](#contributing)
  - [Development workflow](#development-workflow)
  - [Writing high‑impact code changes](#writing-highimpact-code-changes)
  - [Opening a pull request](#opening-a-pull-request)
  - [Review process](#review-process)
  - [Community values](#community-values)
  - [Getting help](#getting-help)
  - [Developer Certificate of Origin (DCO)](#developer-certificate-of-origin-dco)
    - [How to sign (recommended flow)](#how-to-sign-recommended-flow)
    - [Quick fixes](#quick-fixes)
- [Security \& Responsible AI](#securityresponsibleai)
- [License](#license)

</details>

---

## Quickstart

Install globally:

```shell
npm install -g @openai/codex
```

Next, set your OpenAI API key as an environment variable:

```bash
export OPENAI_API_KEY="your-api-key-here"
```

> **Note:** This command sets the key only for your current terminal session. To make it permanent, add the `export` line to your shell's configuration file (e.g., `~/.zshrc`).

Run interactively:

```shell
codex
```

Or, run with a prompt as input (and optionally in `Full Auto` mode):

```shell
codex "explain this codebase to me"
```

```shell
codex --approval-mode full-auto "create the fanciest todo-list app"
```

That’s it – Codex will scaffold a file, run it inside a sandbox, install any
missing dependencies, and show you the live result. Approve the changes and
they’ll be committed to your working directory.

---

## Why Codex?

Codex CLI is built for developers who already **live in the terminal** and want
ChatGPT‑level reasoning **plus** the power to actually run code, manipulate
files, and iterate – all under version control. In short, it’s _chat‑driven
development_ that understands and executes your repo.

- **Zero setup** — bring your OpenAI API key and it just works!
- **Full auto-approval, while safe + secure** by running network-disabled and directory-sandboxed
- **Multimodal** — pass in screenshots or diagrams to implement features ✨

And it's **fully open-source** so you can see and contribute to how it develops!

---

## Security Model & Permissions

Codex lets you decide _how much autonomy_ the agent receives and auto-approval policy via the
`--approval-mode` flag (or the interactive onboarding prompt):

| Mode                      | What the agent may do without asking            | Still requires approval                                         |
| ------------------------- | ----------------------------------------------- | --------------------------------------------------------------- |
| **Suggest** <br>(default) | • Read any file in the repo                     | • **All** file writes/patches <br>• **All** shell/Bash commands |
| **Auto Edit**             | • Read **and** apply‑patch writes to files      | • **All** shell/Bash commands                                   |
| **Full Auto**             | • Read/write files <br>• Execute shell commands | –                                                               |

In **Full Auto** every command is run **network‑disabled** and confined to the
current working directory (plus temporary files) for defense‑in‑depth. Codex
will also show a warning/confirmation if you start in **auto‑edit** or
**full‑auto** while the directory is _not_ tracked by Git, so you always have a
safety net.

Coming soon: you’ll be able to whitelist specific commands to auto‑execute with
the network enabled, once we’re confident in additional safeguards.

### Platform sandboxing details

The hardening mechanism Codex uses depends on your OS:

- **macOS 12+** – commands are wrapped with **Apple Seatbelt** (`sandbox-exec`).

  - Everything is placed in a read‑only jail except for a small set of
    writable roots (`$PWD`, `$TMPDIR`, `~/.codex`, etc.).
  - Outbound network is _fully blocked_ by default – even if a child process
    tries to `curl` somewhere it will fail.

- **Linux** – we recommend using Docker for sandboxing, where Codex launches itself inside a **minimal
  container image** and mounts your repo _read/write_ at the same path. A
  custom `iptables`/`ipset` firewall script denies all egress except the
  OpenAI API. This gives you deterministic, reproducible runs without needing
  root on the host. You can read more in [`run_in_container.sh`](./codex-cli/scripts/run_in_container.sh)

Both approaches are _transparent_ to everyday usage – you still run `codex` from your repo root and approve/reject steps as usual.

---

## System Requirements

| Requirement                 | Details                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Operating systems           | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 **via WSL2** |
| Node.js                     | **22 or newer** (LTS recommended)                               |
| Git (optional, recommended) | 2.23+ for built‑in PR helpers                                   |
| RAM                         | 4‑GB minimum (8‑GB recommended)                                 |

> Never run `sudo npm install -g`; fix npm permissions instead.

---

## CLI Reference

| Command        | Purpose                             | Example                              |
| -------------- | ----------------------------------- | ------------------------------------ |
| `codex`        | Interactive REPL                    | `codex`                              |
| `codex "…"`    | Initial prompt for interactive REPL | `codex "fix lint errors"`            |
| `codex -q "…"` | Non‑interactive "quiet mode"        | `codex -q --json "explain utils.ts"` |

Key flags: `--model/-m`, `--approval-mode/-a`, and `--quiet/-q`.

---

## Memory & Project Docs

Codex merges Markdown instructions in this order:

1. `~/.codex/instructions.md` – personal global guidance
2. `codex.md` at repo root – shared project notes
3. `codex.md` in cwd – sub‑package specifics

Disable with `--no-project-doc` or `CODEX_DISABLE_PROJECT_DOC=1`.

---

## Non‑interactive / CI mode

Run Codex head‑less in pipelines. Example GitHub Action step:

```yaml
- name: Update changelog via Codex
  run: |
    npm install -g @openai/codex
    export OPENAI_API_KEY="${{ secrets.OPENAI_KEY }}"
    codex -a auto-edit --quiet "update CHANGELOG for next release"
```

Set `CODEX_QUIET_MODE=1` to silence interactive UI noise.

---

## Recipes

Below are a few bite‑size examples you can copy‑paste. Replace the text in quotes with your own task.

| ✨  | What you type                                                                   | What happens                                                               |
| --- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 1   | `codex "Refactor the Dashboard component to React Hooks"`                       | Codex rewrites the class component, runs `npm test`, and shows the diff.   |
| 2   | `codex "Generate SQL migrations for adding a users table"`                      | Infers your ORM, creates migration files, and runs them in a sandboxed DB. |
| 3   | `codex "Write unit tests for utils/date.ts"`                                    | Generates tests, executes them, and iterates until they pass.              |
| 4   | `codex "Bulk‑rename *.jpeg → *.jpg with git mv"`                                | Safely renames files and updates imports/usages.                           |
| 5   | `codex "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`                      | Outputs a step‑by‑step human explanation.                                  |
| 6   | `codex "Carefully review this repo, and propose 3 high impact well-scoped PRs"` | Suggests impactful PRs in the current codebase.                            |

---

## Installation

<details open>
<summary><strong>From npm (Recommended)</strong></summary>

```bash
npm install -g @openai/codex
# or
yarn global add @openai/codex
```

</details>

<details>
<summary><strong>Build from source</strong></summary>

```bash
# Clone the repository and navigate to the CLI package
git clone https://github.com/openai/codex.git
cd codex/codex-cli

# Install dependencies and build
npm install
npm run build

# Run the locally‑built CLI directly
node ./dist/cli.js --help

# Or link the command globally for convenience
npm link
```

</details>

---

## Configuration

Codex looks for config files in **`~/.codex/`**.

```yaml
# ~/.codex/config.yaml
model: o4-mini # Default model
fullAutoErrorMode: ask-user # or ignore-and-continue
```

You can also define custom instructions:

```yaml
# ~/.codex/instructions.md
- Always respond with emojis
- Only use git commands if I explicitly mention you should
```

---

## FAQ

<details>
<summary>How do I stop Codex from touching my repo?</summary>

Codex always runs in a **sandbox first**. If a proposed command or file change looks suspicious you can simply answer **n** when prompted and nothing happens to your working tree.

</details>

<details>
<summary>Does it work on Windows?</summary>

Not directly, it requires [Linux on Windows (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install) – Codex is tested on macOS and Linux with Node ≥ 22.

</details>

<details>
<summary>Which models are supported?</summary>

Any model available with [Responses API](https://platform.openai.com/docs/api-reference/responses). The default is `o4-mini`, but pass `--model gpt-4o` or set `model: gpt-4o` in your config file to override.

</details>

---

## Contributing

This project is under active development and the code will likely change pretty significantly. We'll update this message once that's complete!

More broadly We welcome contributions – whether you are opening your very first pull request or you’re a seasoned maintainer. At the same time we care about reliability and long‑term maintainability, so the bar for merging code is intentionally **high**. The guidelines below spell out what “high‑quality” means in practice and should make the whole process transparent and friendly.

<details>
<summary><strong>Codebase Overview (generated by Codex)</strong></summary>
1. Project layout

- package.json – defines the “codex” CLI (bundled to `dist/cli.js`), scripts (build, test, lint, etc.), dependencies (OpenAI SDK, Ink/React, meow, chalk, express, etc.) and dev‑dependencies (TypeScript, Vitest, ESLint, Prettier, esbuild).
- build.mjs – an esbuild script that bundles `src/cli.tsx` → `dist/cli.js` (and a dev variant `cli‑dev.js`), strips out an incompatible React‑Devtools import,
  and injects a `require` shim for CJS modules.
- src/ – the heart of the tool (all `.ts`/`.tsx` source):

  - `src/cli.tsx` – the entrypoint. Parses args with meow, sets up OpenAI API key, modes (interactive, quiet, full‑context), and then either invokes
    `runSinglePass`, `runQuietMode`, or renders the Ink TUI.
  - `src/app.tsx` – top‑level Ink component. If you’re inside a Git repo it hands off to `<TerminalChat/>`; if not, it forces you to confirm you really want to run
    outside version control.
  - `src/cli_singlepass.tsx` – the one‑shot “full‑context” editing mode, which gives the model your entire repo and a prompt and then exits.
  - `src/components/…` – reusable React/Ink components for chat UIs, single‑pass UI, command confirmations, etc.
  - `src/utils/…` – support modules: - config.ts – load and persist `~/.codex/config.{json,yml}`, merge in project docs (`codex.md`), handle timeouts and base URLs. - agent/agent‑loop.ts – the core “agent” engine. Streams responses from OpenAI, turns function calls into shell commands or `apply_patch` calls, enforces
    cancellation/termination, and calls back to the UI. - agent/log.ts – debug logging. - agent/review.ts – handling user decisions (continue, reject, etc.). - auto‑approval‑mode.ts – maps CLI flags (`--full-auto`, `--auto-edit`) → approval policy. - model‑utils.js – check your account’s supported models, preload embeddings/models. - parsers.ts – parse the “tool calls” (e.g. `["bash","-lc","…"]` or `apply_patch`) out of function_call items.
  - terminal.ts – setup/cleanup, raw‑mode key handling, signal handlers.
  - `src/lib/…` – standalone helpers for parsing & formatting patches, shell‑quote safety analysis, text buffers, etc. These are imported in many places via the TS
    path alias `@lib/*`.
  - `src/typings.d.ts` – any ambient type overrides.

- dist/ – bundled CLI artifact, published to NPM (and consumed by the Dockerfile).
- tests/ – ~100 Vitest suites covering the agent logic, patch parsing, safety rules, UI behavior (Ink Testing Library), multiline input, config, markdown
  rendering, etc.
- examples/ – small demo projects (e.g. `camerascii`, `prompt-analyzer`, a “build‑codex‑demo”, plus a prompting guide).
- scripts/ – helper shell scripts (container build, firewall setup, run‑in‑container).
- Dockerfile – a Node 20 image that installs shell tooling, copies in the built `codex.tgz`, and sets up a minimal sandbox firewall script.

2. Core execution modes
   - Interactive (default):
     - `codex <prompt>` launches an Ink/React TUI (`<TerminalChat/>`)
     - Multi‑turn chat, model issues “function calls” for edits or shell commands, user is prompted to approve or reject.
     - Supports flags: `--auto-edit`, `--full-auto`, `--no-project-doc`, image inputs (`-i`), project doc overrides, quiet/full‑stdout toggles.
     - Quiet (`-q, --quiet`): no TUI, just streams the final assistant messages or JSON.
     - Full‑context (`-f, --full-context`): single‑pass editing mode, feeds the entire repo to the model and applies a batch of edits in one go, then exits.
3. Build & test
   - `npm run build` → `build.mjs` → `dist/cli.js`
   - `npm test` runs Vitest suites.
   - `npm run lint` / `npm run format` enforce code style via ESLint/Prettier.
   - Type‑checking is done with `tsc --noEmit`.
4. High‑level workflow
   - User runs `codex “…”`.
   - CLI parses flags, loads user config (~/.codex), discovers `codex.md` in repo.
   - In interactive mode, `<App/>` checks you’re in Git, then spin up an `AgentLoop`.
   - `AgentLoop` streams chat completions, emits `function_call` items.
   - Each call is parsed (edit vs shell), run through `canAutoApprove` → user prompts or auto‑approval.
   - Approved commands are executed (`apply_patch` for file edits, `child_process` for shell commands). Outputs are fed back to the loop as function_call_output
     items so the model sees the results.
   - Loop continues until the model signals it’s done.

</details>

### Development workflow

- Create a _topic branch_ from `main` – e.g. `feat/interactive-prompt`.
- Keep your changes focused. Multiple unrelated fixes should be opened as separate PRs.
- Use `npm run test:watch` during development for super‑fast feedback.
- We use **Vitest** for unit tests, **ESLint** + **Prettier** for style, and **TypeScript** for type‑checking.
- Make sure all your commits are signed off with `git commit -s ...`, see [Developer Certificate of Origin (DCO)](#developer-certificate-of-origin-dco) for more details.

```bash
# Watch mode (tests rerun on change)
npm run test:watch

# Type‑check without emitting files
npm run typecheck

# Automatically fix lint + prettier issues
npm run lint:fix
npm run format:fix
```

### Writing high‑impact code changes

1. **Start with an issue.**
   Open a new one or comment on an existing discussion so we can agree on the solution before code is written.
2. **Add or update tests.**
   Every new feature or bug‑fix should come with test coverage that fails before your change and passes afterwards. 100 % coverage is not required, but aim for meaningful assertions.
3. **Document behaviour.**
   If your change affects user‑facing behaviour, update the README, inline help (`codex --help`), or relevant example projects.
4. **Keep commits atomic.**
   Each commit should compile and the tests should pass. This makes reviews and potential rollbacks easier.

### Opening a pull request

- Fill in the PR template (or include similar information) – **What? Why? How?**
- Run **all** checks locally (`npm test && npm run lint && npm run typecheck`).
  CI failures that could have been caught locally slow down the process.
- Make sure your branch is up‑to‑date with `main` and that you have resolved merge conflicts.
- Mark the PR as **Ready for review** only when you believe it is in a merge‑able state.

### Review process

1. One maintainer will be assigned as a primary reviewer.
2. We may ask for changes – please do not take this personally. We value the work, we just also value consistency and long‑term maintainability.
3. When there is consensus that the PR meets the bar, a maintainer will squash‑and‑merge.

### Community values

- **Be kind and inclusive.** Treat others with respect; we follow the [Contributor Covenant](https://www.contributor-covenant.org/).
- **Assume good intent.** Written communication is hard – err on the side of generosity.
- **Teach & learn.** If you spot something confusing, open an issue or PR with improvements.

### Getting help

If you run into problems setting up the project, would like feedback on an idea, or just want to say _hi_ – please open a Discussion or jump into the relevant issue. We are happy to help.

Together we can make Codex CLI an incredible tool. **Happy hacking!** :rocket:

### Developer Certificate of Origin (DCO)

All commits **must** include a `Signed‑off‑by:` footer.  
This one‑line self‑certification tells us you wrote the code and can contribute it under the repo’s license.

#### How to sign (recommended flow)

```bash
# squash your work into ONE signed commit
git reset --soft origin/main          # stage all changes
git commit -s -m "Your concise message"
git push --force-with-lease           # updates the PR
```

> We enforce **squash‑and‑merge only**, so a single signed commit is enough for the whole PR.

#### Quick fixes

| Scenario          | Command                                                                                   |
| ----------------- | ----------------------------------------------------------------------------------------- |
| Amend last commit | `git commit --amend -s --no-edit && git push -f`                                          |
| GitHub UI only    | Edit the commit message in the PR → add<br>`Signed-off-by: Your Name <email@example.com>` |

The **DCO check** blocks merges until every commit in the PR carries the footer (with squash this is just the one).

---

## Security &amp; Responsible AI

Have you discovered a vulnerability or have concerns about model output? Please e‑mail **security@openai.com** and we will respond promptly.

---

## License

This repository is licensed under the [Apache-2.0 License](LICENSE).
