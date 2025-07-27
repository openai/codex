<h1 align="center">OpenAI Codex CLI</h1>
<p align="center">Lightweight coding agent that runs in your terminal</p>

<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install codex</code></p>

This is the home of the **Codex CLI**, which is a coding agent from OpenAI that runs locally on your computer. If you are looking for the _cloud-based agent_ from OpenAI, **Codex [Web]**, see <https://chatgpt.com/codex>.

<!-- ![Codex demo GIF using: codex "explain this codebase to me"](./.github/demo.gif) -->

---

<details>
<summary><strong>Table of contents</strong></summary>

<!-- Begin ToC -->

- [Experimental technology disclaimer](#experimental-technology-disclaimer)
- [Quickstart](#quickstart)
  - [OpenAI API Users](#openai-api-users)
  - [OpenAI Plus/Pro Users](#openai-pluspro-users)
- [Why Codex?](#why-codex)
- [Security model & permissions](#security-model--permissions)
  - [Platform sandboxing details](#platform-sandboxing-details)
- [System requirements](#system-requirements)
- [CLI reference](#cli-reference)
- [Memory & project docs](#memory--project-docs)
- [Non-interactive / CI mode](#non-interactive--ci-mode)
- [Model Context Protocol (MCP)](#model-context-protocol-mcp)
- [Tracing / verbose logging](#tracing--verbose-logging)
- [Recipes](#recipes)
- [Installation](#installation)
  - [DotSlash](#dotslash)
- [Configuration](#configuration)
- [FAQ](#faq)
- [Zero data retention (ZDR) usage](#zero-data-retention-zdr-usage)
- [Codex open source fund](#codex-open-source-fund)
- [Contributing](#contributing)
  - [Development workflow](#development-workflow)
  - [Writing high-impact code changes](#writing-high-impact-code-changes)
  - [Opening a pull request](#opening-a-pull-request)
  - [Review process](#review-process)
  - [Community values](#community-values)
  - [Getting help](#getting-help)
  - [Contributor license agreement (CLA)](#contributor-license-agreement-cla)
    - [Quick fixes](#quick-fixes)
  - [Releasing `codex`](#releasing-codex)
- [Security & responsible AI](#security--responsible-ai)
- [License](#license)

<!-- End ToC -->

</details>

---

## Experimental technology disclaimer

Codex CLI is an experimental project under active development. It is not yet stable, may contain bugs, incomplete features, or undergo breaking changes. We're building it in the open with the community and welcome:

- Bug reports
- Feature requests
- Pull requests
- Good vibes

Help us improve by filing issues or submitting PRs (see the section below for how to contribute)!

## Quickstart

Install globally with your preferred package manager:

```shell
npm install -g @openai/codex  # Alternatively: `brew install codex`
```

Or go to the [latest GitHub Release](https://github.com/openai/codex/releases/latest) and download the appropriate binary for your platform.

### OpenAI API Users

Next, set your OpenAI API key as an environment variable:

```shell
export OPENAI_API_KEY="your-api-key-here"
```

> [!NOTE]
> This command sets the key only for your current terminal session. You can add the `export` line to your shell's configuration file (e.g., `~/.zshrc`), but we recommend setting it for the session.

### OpenAI Plus/Pro Users

If you have a paid OpenAI account, run the following to start the login process:

```
codex login
```

If you complete the process successfully, you should have a `~/.codex/auth.json` file that contains the credentials that Codex will use.

If you encounter problems with the login flow, please comment on <https://github.com/openai/codex/issues/1243>.

<details>
<summary><strong>Use <code>--profile</code> to use other models</strong></summary>

Codex also allows you to use other providers that support the OpenAI Chat Completions (or Responses) API.

To do so, you must first define custom [providers](./config.md#model_providers) in `~/.codex/config.toml`. For example, the provider for a standard Ollama setup would be defined as follows:

```toml
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"
```

The `base_url` will have `/chat/completions` appended to it to build the full URL for the request.

For providers that also require an `Authorization` header of the form `Bearer: SECRET`, an `env_key` can be specified, which indicates the environment variable to read to use as the value of `SECRET` when making a request:

```toml
[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
```

Providers that speak the Responses API are also supported by adding `wire_api = "responses"` as part of the definition. Accessing OpenAI models via Azure is an example of such a provider, though it also requires specifying additional `query_params` that need to be appended to the request URL:

```toml
[model_providers.azure]
name = "Azure"
# Make sure you set the appropriate subdomain for this URL.
base_url = "https://YOUR_PROJECT_NAME.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"  # Or "OPENAI_API_KEY", whichever you use.
# Newer versions appear to support the responses API, see https://github.com/openai/codex/pull/1321
query_params = { api-version = "2025-04-01-preview" }
wire_api = "responses"
```

Once you have defined a provider you wish to use, you can configure it as your default provider as follows:

```toml
model_provider = "azure"
```

> [!TIP]
> If you find yourself experimenting with a variety of models and providers, then you likely want to invest in defining a _profile_ for each configuration like so:

```toml
[profiles.o3]
model_provider = "azure"
model = "o3"

[profiles.mistral]
model_provider = "ollama"
model = "mistral"
```

This way, you can specify one command-line argument (.e.g., `--profile o3`, `--profile mistral`) to override multiple settings together.

</details>
<br />

Run interactively:

```shell
codex
```

Or, run with a prompt as input (and optionally in `Full Auto` mode):

```shell
codex "explain this codebase to me"
```

```shell
codex --full-auto "create the fanciest todo-list app"
```

That's it - Codex will scaffold a file, run it inside a sandbox, install any
missing dependencies, and show you the live result. Approve the changes and
they'll be committed to your working directory.

---

## Why Codex?

Codex CLI is built for developers who already **live in the terminal** and want
ChatGPT-level reasoning **plus** the power to actually run code, manipulate
files, and iterate - all under version control. In short, it's _chat-driven
development_ that understands and executes your repo.

- **Zero setup** - bring your OpenAI API key and it just works!
- **Full auto-approval, while safe + secure** by running network-disabled and directory-sandboxed
- **Multimodal** - pass in screenshots or diagrams to implement features ✨

And it's **fully open-source** so you can see and contribute to how it develops!

---

## Security model & permissions

Codex lets you decide _how much autonomy_ you want to grant the agent. The following options can be configured independently:

- [`approval_policy`](./codex-rs/config.md#approval_policy) determines when you should be prompted to approve whether Codex can execute a command
- [`sandbox`](./codex-rs/config.md#sandbox) determines the _sandbox policy_ that Codex uses to execute untrusted commands

By default, Codex runs with `--ask-for-approval untrusted` and `--sandbox read-only`, which means that:

- The user is prompted to approve every command not on the set of "trusted" commands built into Codex (`cat`, `ls`, etc.)
- Approved commands are run outside of a sandbox because user approval implies "trust," in this case.

Running Codex with the `--full-auto` convenience flag changes the configuration to `--ask-for-approval on-failure` and `--sandbox workspace-write`, which means that:

- Codex does not initially ask for user approval before running an individual command.
- Though when it runs a command, it is run under a sandbox in which:
  - It can read any file on the system.
  - It can only write files under the current directory (or the directory specified via `--cd`).
  - Network requests are completely disabled.
- Only if the command exits with a non-zero exit code will it ask the user for approval. If granted, it will re-attempt the command outside of the sandbox. (A common case is when Codex cannot `npm install` a dependency because that requires network access.)

Again, these two options can be configured independently. For example, if you want Codex to perform an "exploration" where you are happy for it to read anything it wants but you never want to be prompted, you could run Codex with `--ask-for-approval never` and `--sandbox read-only`.

### Platform sandboxing details

The mechanism Codex uses to implement the sandbox policy depends on your OS:

- **macOS 12+** uses **Apple Seatbelt** and runs commands using `sandbox-exec` with a profile (`-p`) that corresponds to the `--sandbox` that was specified.
- **Linux** uses a combination of Landlock/seccomp APIs to enforce the `sandbox` configuration.

Note that when running Linux in a containerized environment such as Docker, sandboxing may not work if the host/container configuration does not support the necessary Landlock/seccomp APIs. In such cases, we recommend configuring your Docker container so that it provides the sandbox guarantees you are looking for and then running `codex` with `--sandbox danger-full-access` (or, more simply, the `--dangerously-bypass-approvals-and-sandbox` flag) within your container.

---

## System requirements

| Requirement                 | Details                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Operating systems           | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 **via WSL2** |
| Git (optional, recommended) | 2.23+ for built-in PR helpers                                   |
| RAM                         | 4-GB minimum (8-GB recommended)                                 |

---

## CLI reference

| Command            | Purpose                            | Example                         |
| ------------------ | ---------------------------------- | ------------------------------- |
| `codex`            | Interactive TUI                    | `codex`                         |
| `codex "..."`      | Initial prompt for interactive TUI | `codex "fix lint errors"`       |
| `codex exec "..."` | Non-interactive "automation mode"  | `codex exec "explain utils.ts"` |

Key flags: `--model/-m`, `--ask-for-approval/-a`.

---

## Memory & project docs

You can give Codex extra instructions and guidance using `AGENTS.md` files. Codex looks for `AGENTS.md` files in the following places, and merges them top-down:

1. `~/.codex/AGENTS.md` - personal global guidance
2. `AGENTS.md` at repo root - shared project notes
3. `AGENTS.md` in the current working directory - sub-folder/feature specifics

---

## Non-interactive / CI mode

Run Codex head-less in pipelines. Example GitHub Action step:

```yaml
- name: Update changelog via Codex
  run: |
    npm install -g @openai/codex
    export OPENAI_API_KEY="${{ secrets.OPENAI_KEY }}"
    codex exec --full-auto "update CHANGELOG for next release"
```

## Model Context Protocol (MCP)

The Codex CLI can be configured to leverage MCP servers by defining an [`mcp_servers`](./codex-rs/config.md#mcp_servers) section in `~/.codex/config.toml`. It is intended to mirror how tools such as Claude and Cursor define `mcpServers` in their respective JSON config files, though the Codex format is slightly different since it uses TOML rather than JSON, e.g.:

```toml
# IMPORTANT: the top-level key is `mcp_servers` rather than `mcpServers`.
[mcp_servers.server-name]
command = "npx"
args = ["-y", "mcp-server"]
env = { "API_KEY" = "value" }
```

> [!TIP]
> It is somewhat experimental, but the Codex CLI can also be run as an MCP _server_ via `codex mcp`. If you launch it with an MCP client such as `npx @modelcontextprotocol/inspector codex mcp` and send it a `tools/list` request, you will see that there is only one tool, `codex`, that accepts a grab-bag of inputs, including a catch-all `config` map for anything you might want to override. Feel free to play around with it and provide feedback via GitHub issues.

## Tracing / verbose logging

Because Codex is written in Rust, it honors the `RUST_LOG` environment variable to configure its logging behavior.

The TUI defaults to `RUST_LOG=codex_core=info,codex_tui=info` and log messages are written to `~/.codex/log/codex-tui.log`, so you can leave the following running in a separate terminal to monitor log messages as they are written:

```
tail -F ~/.codex/log/codex-tui.log
```

By comparison, the non-interactive mode (`codex exec`) defaults to `RUST_LOG=error`, but messages are printed inline, so there is no need to monitor a separate file.

See the Rust documentation on [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for more information on the configuration options.

---

## Recipes

Below are a few bite-size examples you can copy-paste. Replace the text in quotes with your own task. See the [prompting guide](https://github.com/openai/codex/blob/main/codex-cli/examples/prompting_guide.md) for more tips and usage patterns.

| ✨  | What you type                                                                   | What happens                                                               |
| --- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 1   | `codex "Refactor the Dashboard component to React Hooks"`                       | Codex rewrites the class component, runs `npm test`, and shows the diff.   |
| 2   | `codex "Generate SQL migrations for adding a users table"`                      | Infers your ORM, creates migration files, and runs them in a sandboxed DB. |
| 3   | `codex "Write unit tests for utils/date.ts"`                                    | Generates tests, executes them, and iterates until they pass.              |
| 4   | `codex "Bulk-rename *.jpeg -> *.jpg with git mv"`                               | Safely renames files and updates imports/usages.                           |
| 5   | `codex "Explain what this regex does: ^(?=.*[A-Z]).{8,}$"`                      | Outputs a step-by-step human explanation.                                  |
| 6   | `codex "Carefully review this repo, and propose 3 high impact well-scoped PRs"` | Suggests impactful PRs in the current codebase.                            |
| 7   | `codex "Look for vulnerabilities and create a security review report"`          | Finds and explains security bugs.                                          |

---

## Installation

<details open>
<summary><strong>Install Codex CLI using your preferred package manager.</strong></summary>

From `brew` (recommended, downloads only the binary for your platform):

```bash
brew install codex
```

From `npm` (generally more readily available, but downloads binaries for all supported platforms):

```bash
npm i -g @openai/codex
```

Or go to the [latest GitHub Release](https://github.com/openai/codex/releases/latest) and download the appropriate binary for your platform.

Admittedly, each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

### DotSlash

The GitHub Release also contains a [DotSlash](https://dotslash-cli.com/) file for the Codex CLI named `codex`. Using a DotSlash file makes it possible to make a lightweight commit to source control to ensure all contributors use the same version of an executable, regardless of what platform they use for development.

</details>

<details>
<summary><strong>Build from source</strong></summary>

```bash
# Clone the repository and navigate to the root of the Cargo workspace.
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Install the Rust toolchain, if necessary.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup component add rustfmt
rustup component add clippy

# Build Codex.
cargo build

# Launch the TUI with a sample prompt.
cargo run --bin codex -- "explain this codebase to me"

# After making changes, ensure the code is clean.
cargo fmt -- --config imports_granularity=Item
cargo clippy --tests

# Run the tests.
cargo test
```

</details>

---

## Configuration

Codex supports a rich set of configuration options documented in [`codex-rs/config.md`](./codex-rs/config.md).

By default, Codex loads its configuration from `~/.codex/config.toml`.

Though `--config` can be used to set/override ad-hoc config values for individual invocations of `codex`.

---

## FAQ

<details>
<summary>OpenAI released a model called Codex in 2021 - is this related?</summary>

In 2021, OpenAI released Codex, an AI system designed to generate code from natural language prompts. That original Codex model was deprecated as of March 2023 and is separate from the CLI tool.

</details>

<details>
<summary>Which models are supported?</summary>

Any model available with [Responses API](https://platform.openai.com/docs/api-reference/responses). The default is `o4-mini`, but pass `--model gpt-4.1` or set `model: gpt-4.1` in your config file to override.

</details>
<details>
<summary>Why does <code>o3</code> or <code>o4-mini</code> not work for me?</summary>

It's possible that your [API account needs to be verified](https://help.openai.com/en/articles/10910291-api-organization-verification) in order to start streaming responses and seeing chain of thought summaries from the API. If you're still running into issues, please let us know!

</details>

<details>
<summary>How do I stop Codex from editing my files?</summary>

Codex runs model-generated commands in a sandbox. If a proposed command or file change doesn't look right, you can simply type **n** to deny the command or give the model feedback.

</details>
<details>
<summary>Does it work on Windows?</summary>

Not directly. It requires [Windows Subsystem for Linux (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install) - Codex has been tested on macOS and Linux with Node 22.

</details>

---

## Zero data retention (ZDR) usage

Codex CLI **does** support OpenAI organizations with [Zero Data Retention (ZDR)](https://platform.openai.com/docs/guides/your-data#zero-data-retention) enabled. If your OpenAI organization has Zero Data Retention enabled and you still encounter errors such as:

```
OpenAI rejected the request. Error details: Status: 400, Code: unsupported_parameter, Type: invalid_request_error, Message: 400 Previous response cannot be used for this organization due to Zero Data Retention.
```

Ensure you are running `codex` with `--config disable_response_storage=true` or add this line to `~/.codex/config.toml` to avoid specifying the command line option each time:

```toml
disable_response_storage = true
```

See [the configuration documentation on `disable_response_storage`](./codex-rs/config.md#disable_response_storage) for details.

---

## Codex open source fund

We're excited to launch a **$1 million initiative** supporting open source projects that use Codex CLI and other OpenAI models.

- Grants are awarded up to **$25,000** API credits.
- Applications are reviewed **on a rolling basis**.

**Interested? [Apply here](https://openai.com/form/codex-open-source-fund/).**

---

## Contributing

This project is under active development and the code will likely change pretty significantly. We'll update this message once that's complete!

More broadly we welcome contributions - whether you are opening your very first pull request or you're a seasoned maintainer. At the same time we care about reliability and long-term maintainability, so the bar for merging code is intentionally **high**. The guidelines below spell out what "high-quality" means in practice and should make the whole process transparent and friendly.

### Development workflow

- Create a _topic branch_ from `main` - e.g. `feat/interactive-prompt`.
- Keep your changes focused. Multiple unrelated fixes should be opened as separate PRs.
- Following the [development setup](#development-workflow) instructions above, ensure your change is free of lint warnings and test failures.

### Writing high-impact code changes

1. **Start with an issue.** Open a new one or comment on an existing discussion so we can agree on the solution before code is written.
2. **Add or update tests.** Every new feature or bug-fix should come with test coverage that fails before your change and passes afterwards. 100% coverage is not required, but aim for meaningful assertions.
3. **Document behaviour.** If your change affects user-facing behaviour, update the README, inline help (`codex --help`), or relevant example projects.
4. **Keep commits atomic.** Each commit should compile and the tests should pass. This makes reviews and potential rollbacks easier.

### Opening a pull request

- Fill in the PR template (or include similar information) - **What? Why? How?**
- Run **all** checks locally (`cargo test && cargo clippy --tests && cargo fmt -- --config imports_granularity=Item`). CI failures that could have been caught locally slow down the process.
- Make sure your branch is up-to-date with `main` and that you have resolved merge conflicts.
- Mark the PR as **Ready for review** only when you believe it is in a merge-able state.

### Review process

1. One maintainer will be assigned as a primary reviewer.
2. We may ask for changes - please do not take this personally. We value the work, we just also value consistency and long-term maintainability.
3. When there is consensus that the PR meets the bar, a maintainer will squash-and-merge.

### Community values

- **Be kind and inclusive.** Treat others with respect; we follow the [Contributor Covenant](https://www.contributor-covenant.org/).
- **Assume good intent.** Written communication is hard - err on the side of generosity.
- **Teach & learn.** If you spot something confusing, open an issue or PR with improvements.

### Getting help

If you run into problems setting up the project, would like feedback on an idea, or just want to say _hi_ - please open a Discussion or jump into the relevant issue. We are happy to help.

Together we can make Codex CLI an incredible tool. **Happy hacking!** :rocket:

### Contributor license agreement (CLA)

All contributors **must** accept the CLA. The process is lightweight:

1. Open your pull request.
2. Paste the following comment (or reply `recheck` if you've signed before):

   ```text
   I have read the CLA Document and I hereby sign the CLA
   ```

3. The CLA-Assistant bot records your signature in the repo and marks the status check as passed.

No special Git commands, email attachments, or commit footers required.

#### Quick fixes

| Scenario          | Command                                          |
| ----------------- | ------------------------------------------------ |
| Amend last commit | `git commit --amend -s --no-edit && git push -f` |

The **DCO check** blocks merges until every commit in the PR carries the footer (with squash this is just the one).

### Releasing `codex`

_For admins only._

Make sure you are on `main` and have no local changes. Then run:

```shell
VERSION=0.2.0  # Can also be 0.2.0-alpha.1 or any valid Rust version.
./codex-rs/scripts/create_github_release.sh "$VERSION"
```

This will make a local commit on top of `main` with `version` set to `$VERSION` in `codex-rs/Cargo.toml` (note that on `main`, we leave the version as `version = "0.0.0"`).

This will push the commit using the tag `rust-v${VERSION}`, which in turn kicks off [the release workflow](.github/workflows/rust-release.yml). This will create a new GitHub Release named `$VERSION`.

If everything looks good in the generated GitHub Release, uncheck the **pre-release** box so it is the latest release.

Create a PR to update [`Formula/c/codex.rb`](https://github.com/Homebrew/homebrew-core/blob/main/Formula/c/codex.rb) on Homebrew.

---

## Security & responsible AI

Have you discovered a vulnerability or have concerns about model output? Please e-mail **security@openai.com** and we will respond promptly.

---

## License

This repository is licensed under the [Apache-2.0 License](LICENSE).
