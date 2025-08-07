<h1 align="center">OpenAI Codex CLI</h1>
<p align="center">Lightweight coding agent that runs in your terminal</p>

<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install codex</code></p>

This is the home of the **Codex CLI**, OpenAI's coding agent that runs locally on your computer. If you are looking for the _cloud-based agent_ from OpenAI, **Codex [Web]**, see <https://chatgpt.com/codex>.

<!-- ![Codex demo GIF using: codex "explain this codebase to me"](./.github/demo.gif) -->

---

> [!WARNING]
> Experimental technology disclaimer
> Codex CLI is an experimental project under active development. We're building it in the open with the community; Help us improve by filing issues or submitting PRs (see [contributing](./docs/contributing.md) for how to contribute)!

## Quickstart

Install globally with your preferred package manager:

```shell
npm install -g @openai/codex  # Alternatively: `brew install codex`
```

Or go to the [latest GitHub Release](https://github.com/openai/codex/releases/latest) and download the appropriate binary for your platform.

### ChatGPT Plus/Pro Users

If you have a ChatGPT Plus or Pro account, login with:

```
codex login
```

This should create a `~/.codex/auth.json` file that contains the credentials that Codex will use.

To verify whether you are currently logged in, run:

```
codex login status
```

If you encounter problems with the login flow, please comment on <https://github.com/openai/codex/issues/1243>.


### OpenAI API Users

Next, set your OpenAI API key as an environment variable:

```shell
export OPENAI_API_KEY="your-api-key-here"
```

> [!NOTE]
> This command sets the key only for your current terminal session. You can add the `export` line to your shell's configuration file (e.g., `~/.zshrc`), but we recommend setting it for the session.

**Run**

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



## System requirements

| Requirement                 | Details                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Operating systems           | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 **via WSL2** |
| Git (optional, recommended) | 2.23+ for built-in PR helpers                                   |
| RAM                         | 4-GB minimum (8-GB recommended)                                 |

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

---

## Configuration

Codex supports a rich set of configuration options documented in [`codex-rs/config.md`](./codex-rs/config.md).

By default, Codex loads its configuration from `~/.codex/config.toml`. Additionally, `--config` can be used to set/override ad-hoc config values for individual invocations of `codex`.

For more information, see [config.md](./docs/config.md).

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

## Codex open source fund

We're excited to launch a **$1 million initiative** supporting open source projects that use Codex CLI and other OpenAI models.

- Grants are awarded up to **$25,000** API credits.
- Applications are reviewed **on a rolling basis**.

**Interested? [Apply here](https://openai.com/form/codex-open-source-fund/).**

---

## Security & responsible AI

Have you discovered a vulnerability or have concerns about model output? Please e-mail **security@openai.com** and we will respond promptly.

---

## License

This repository is licensed under the [Apache-2.0 License](LICENSE).
