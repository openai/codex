<h1 align="center">OpenAI Codex CLI</h1>

<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install codex</code></p>

<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, see <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

<p align="center">
  <img src="./.github/codex-cli-splash.png" alt="Codex CLI splash" width="50%" />
  </p>

---

<details>
<summary><strong>Table of contents</strong></summary>

<!-- Begin ToC -->

- [Quickstart](#quickstart)
  - [Installing and running Codex CLI](#installing-and-running-codex-cli)
  - [Using Codex with your ChatGPT plan](#using-codex-with-your-chatgpt-plan)
  - [Connecting on a "Headless" Machine](#connecting-on-a-headless-machine)
    - [Authenticate locally and copy your credentials to the "headless" machine](#authenticate-locally-and-copy-your-credentials-to-the-headless-machine)
    - [Connecting through VPS or remote](#connecting-through-vps-or-remote)
  - [Usage-based billing alternative: Use an OpenAI API key](#usage-based-billing-alternative-use-an-openai-api-key)
    - [Forcing a specific auth method (advanced)](#forcing-a-specific-auth-method-advanced)
  - [Choosing Codex's level of autonomy](#choosing-codexs-level-of-autonomy)
    - [**1. Read/write**](#1-readwrite)
    - [**2. Read-only**](#2-read-only)
    - [**3. Advanced configuration**](#3-advanced-configuration)
    - [Can I run without ANY approvals?](#can-i-run-without-any-approvals)
    - [Fine-tuning in `config.toml`](#fine-tuning-in-configtoml)
  - [Example prompts](#example-prompts)
- [Running with a prompt as input](#running-with-a-prompt-as-input)
- [Using Open Source Models](#using-open-source-models)
  - [Platform sandboxing details](#platform-sandboxing-details)
- [Experimental technology disclaimer](#experimental-technology-disclaimer)
- [System requirements](#system-requirements)
- [CLI reference](#cli-reference)
- [Memory & project docs](#memory--project-docs)
- [Non-interactive / CI mode](#non-interactive--ci-mode)
- [Model Context Protocol (MCP)](#model-context-protocol-mcp)
- [Tracing / verbose logging](#tracing--verbose-logging)
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

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
npm install -g @openai/codex  # Alternatively: `brew install codex`
```

Then simply run `codex` to get started:

```shell
codex
```

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

<p align="center">
  <img src="./.github/codex-cli-login.png" alt="Codex CLI login" width="50%" />
  </p>

Run `codex` and select **Sign in with ChatGPT**. You'll need a Plus, Pro, or Team ChatGPT account, and will get access to our latest models, including `gpt-5`, at no extra cost to your plan. (Enterprise is coming soon.)

> Important: If you've used the Codex CLI before, follow these steps to migrate from usage-based billing with your API key:
>
> 1. Update the CLI and ensure `codex --version` is `0.20.0` or later
> 2. Delete `~/.codex/auth.json` (this should be `C:\Users\USERNAME\.codex\auth.json` on Windows)
> 3. Run `codex login` again

If you encounter problems with the login flow, please comment on [this issue](https://github.com/openai/codex/issues/1243).

### Connecting on a "Headless" Machine

Today, the login process entails running a server on `localhost:1455`. If you are on a "headless" server, such as a Docker container or are `ssh`'d into a remote machine, loading `localhost:1455` in the browser on your local machine will not automatically connect to the webserver running on the _headless_ machine, so you must use one of the following workarounds:

#### Authenticate locally and copy your credentials to the "headless" machine

The easiest solution is likely to run through the `codex login` process on your local machine such that `localhost:1455` _is_ accessible in your web browser. When you complete the authentication process, an `auth.json` file should be available at `$CODEX_HOME/auth.json` (on Mac/Linux, `$CODEX_HOME` defaults to `~/.codex` whereas on Windows, it defaults to `%USERPROFILE%\.codex`).

Because the `auth.json` file is not tied to a specific host, once you complete the authentication flow locally, you can copy the `$CODEX_HOME/auth.json` file to the headless machine and then `codex` should "just work" on that machine. Note to copy a file to a Docker container, you can do:

```shell
# substitute MY_CONTAINER with the name or id of your Docker container:
CONTAINER_HOME=$(docker exec MY_CONTAINER printenv HOME)
docker exec MY_CONTAINER mkdir -p "$CONTAINER_HOME/.codex"
docker cp auth.json MY_CONTAINER:"$CONTAINER_HOME/.codex/auth.json"
```

whereas if you are `ssh`'d into a remote machine, you likely want to use [`scp`](https://en.wikipedia.org/wiki/Secure_copy_protocol):

```shell
ssh user@remote 'mkdir -p ~/.codex'
scp ~/.codex/auth.json user@remote:~/.codex/auth.json
```

or try this one-liner:

```shell
ssh user@remote 'mkdir -p ~/.codex && cat > ~/.codex/auth.json' < ~/.codex/auth.json
```

#### Connecting through VPS or remote

If you run Codex on a remote machine (VPS/server) without a local browser, the login helper starts a server on `localhost:1455` on the remote host. To complete login in your local browser, forward that port to your machine before starting the login flow:

```bash
# From your local machine
ssh -L 1455:localhost:1455 <user>@<remote-host>
```

Then, in that SSH session, run `codex` and select "Sign in with ChatGPT". When prompted, open the printed URL (it will be `http://localhost:1455/...`) in your local browser. The traffic will be tunneled to the remote server.

### Usage-based billing alternative: Use an OpenAI API key

If you prefer to pay-as-you-go, you can still authenticate with your OpenAI API key by setting it as an environment variable:

```shell
export OPENAI_API_KEY="your-api-key-here"
```

Notes:

- This command only sets the key for your current terminal session, which we recommend. To set it for all future sessions, you can also add the `export` line to your shell's configuration file (e.g., `~/.zshrc`).
- If you have signed in with ChatGPT, Codex will default to using your ChatGPT credits. If you wish to use your API key, use the `/logout` command to clear your ChatGPT authentication.

#### Forcing a specific auth method (advanced)

You can explicitly choose which authentication Codex should prefer when both are available.

- To always use your API key (even when ChatGPT auth exists), set:

```toml
# ~/.codex/config.toml
preferred_auth_method = "apikey"
```

Or override ad-hoc via CLI:

```bash
codex --config preferred_auth_method="apikey"
```

- To prefer ChatGPT auth (default), set:

```toml
# ~/.codex/config.toml
preferred_auth_method = "chatgpt"
```

Notes:

- When `preferred_auth_method = "apikey"` and an API key is available, the login screen is skipped.
- When `preferred_auth_method = "chatgpt"` (default), Codex prefers ChatGPT auth if present; if only an API key is present, it will use the API key. Certain account types may also require API-key mode.

### Choosing Codex's level of autonomy

We always recommend running Codex in its default sandbox that gives you strong guardrails around what the agent can do. The default sandbox prevents it from editing files outside its workspace, or from accessing the network.

When you launch Codex in a new folder, it detects whether the folder is version controlled and recommends one of two levels of autonomy:

#### **1. Read/write**

- Codex can run commands and write files in the workspace without approval.
- To write files in other folders, access network, update git or perform other actions protected by the sandbox, Codex will need your permission.
- By default, the workspace includes the current directory, as well as temporary directories like `/tmp`. You can see what directories are in the workspace with the `/status` command. See the docs for how to customize this behavior.
- Advanced: You can manually specify this configuration by running `codex --sandbox workspace-write --ask-for-approval on-request`
- This is the recommended default for version-controlled folders.

#### **2. Read-only**

- Codex can run read-only commands without approval.
- To edit files, access network, or perform other actions protected by the sandbox, Codex will need your permission.
- Advanced: You can manually specify this configuration by running `codex --sandbox read-only --ask-for-approval on-request`
- This is the recommended default non-version-controlled folders.

#### **3. Advanced configuration**

Codex gives you fine-grained control over the sandbox with the `--sandbox` option, and over when it requests approval with the `--ask-for-approval` option. Run `codex help` for more on these options.

#### Can I run without ANY approvals?

Yes, run codex non-interactively with `--ask-for-approval never`. This option works with all `--sandbox` options, so you still have full control over Codex's level of autonomy. It will make its best attempt with whatever contrainsts you provide. For example:

- Use `codex --ask-for-approval never --sandbox read-only` when you are running many agents to answer questions in parallel in the same workspace.
- Use `codex --ask-for-approval never --sandbox workspace-write` when you want the agent to non-interactively take time to produce the best outcome, with strong guardrails around its behavior.
- Use `codex --ask-for-approval never --sandbox danger-full-access` to dangerously give the agent full autonomy. Because this disables important safety mechanisms, we recommend against using this unless running Codex in an isolated environment.

#### Fine-tuning in `config.toml`

```toml
# approval mode
approval_policy = "untrusted"
sandbox_mode    = "read-only"

# full-auto mode
approval_policy = "on-request"
sandbox_mode    = "workspace-write"

# Optional: allow network in workspace-write mode
[sandbox_workspace_write]
network_access = true
```

You can also save presets as **profiles**:

```toml
[profiles.full_auto]
approval_policy = "on-request"
sandbox_mode    = "workspace-write"

[profiles.readonly_quiet]
approval_policy = "never"
sandbox_mode    = "read-only"
```

### Example prompts

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

## Running with a prompt as input

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

## Using Open Source Models

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

Codex can run fully locally against an OpenAI-compatible OSS host (like Ollama) using the `--oss` flag:

- Interactive UI:
  - codex --oss
- Non-interactive (programmatic) mode:
  - echo "Refactor utils" | codex exec --oss

Model selection when using `--oss`:

- If you omit `-m/--model`, Codex defaults to -m gpt-oss:20b and will verify it exists locally (downloading if needed).
- To pick a different size, pass one of:
  - -m "gpt-oss:20b"
  - -m "gpt-oss:120b"

Point Codex at your own OSS host:

- By default, `--oss` talks to http://localhost:11434/v1.
- To use a different host, set one of these environment variables before running Codex:
  - CODEX_OSS_BASE_URL, for example:
    - CODEX_OSS_BASE_URL="http://my-ollama.example.com:11434/v1" codex --oss -m gpt-oss:20b
  - or CODEX_OSS_PORT (when the host is localhost):
    - CODEX_OSS_PORT=11434 codex --oss

Advanced: you can persist this in your config instead of environment variables by overriding the built-in `oss` provider in `~/.codex/config.toml`:

```toml
[model_providers.oss]
name = "Open Source"
base_url = "http://my-ollama.example.com:11434/v1"
```

---

### Platform sandboxing details

By default, Codex CLI runs code and shell commands inside a restricted sandbox to protect your system.  

> [!IMPORTANT]
> Not all tool calls are sandboxed. Specifically, **trusted Model Context Protocol (MCP) tool calls** are executed outside of the sandbox.  
> This is intentional: MCP tools are explicitly configured and trusted by you, and they often need to connect to **external applications or services** (e.g. issue trackers, databases, messaging systems).  
> Running them outside the sandbox allows Codex to integrate with these external systems without being blocked by sandbox restrictions.

The mechanism Codex uses to implement the sandbox policy depends on your OS:

- **macOS 12+** uses **Apple Seatbelt** and runs commands using `sandbox-exec` with a profile (`-p`) that corresponds to the `--sandbox` that was specified.
- **Linux** uses a combination of Landlock/seccomp APIs to enforce the `sandbox` configuration.

Note that when running Linux in a containerized environment such as Docker, sandboxing may not work if the host/container configuration does not support the necessary Landlock/seccomp APIs. In such cases, we recommend configuring your Docker container so that it provides the sandbox guarantees you are looking for and then running `codex` with `--sandbox danger-full-access` (or, more simply, the `--dangerously-bypass-approvals-and-sandbox` flag) within your container.

---

## Experimental technology disclaimer

Codex CLI is an experimental project under active development. It is not yet stable, may contain bugs, incomplete features, or undergo breaking changes. We're building it in the open with the community and welcome:

- Bug reports
- Feature requests
- Pull requests
- Good vibes

Help us improve by filing issues or submitting PRs (see the section below for how to contribute)!

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

This project is under active development and the code will likely change pretty significantly.

**At the moment, we only plan to prioritize reviewing external contributions for bugs or security fixes.**

If you want to add a new feature or change the behavior of an existing one, please open an issue proposing the feature and get approval from an OpenAI team member before spending time building it.

**New contributions that don't go through this process may be closed** if they aren't aligned with our current roadmap or conflict with other priorities/upcoming features.

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
2. If your PR adds a new feature that was not previously discussed and approved, we may choose to close your PR (see [Contributing](#contributing)).
3. We may ask for changes - please do not take this personally. We value the work, but we also value consistency and long-term maintainability.
5. When there is consensus that the PR meets the bar, a maintainer will squash-and-merge.

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
