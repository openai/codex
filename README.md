<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install --cask codex</code></p>
<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

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
<details>
<summary>Does it work on Windows?</summary>

Not directly. It requires [Windows Subsystem for Linux (WSL2)](https://learn.microsoft.com/en-us/windows/wsl/install) - Codex has been tested on macOS and Linux with Node 22.

</details>

<details>
<summary>PowerShell says <code>npm</code> or <code>codex</code> is not recognized. How do I fix that?</summary>

`npm` is bundled with Node.js. If PowerShell says `npm` is not recognized, Node.js is not installed in that environment yet.

For Codex on Windows, use WSL2 and run install commands inside your Linux shell (not in PowerShell):

1. In **PowerShell (Admin)**, install WSL and reboot if prompted:

   ```powershell
   wsl --install
   ```

2. Open your WSL distro (for example, Ubuntu) and install Node.js 22+.
3. In that same WSL shell, install Codex:

   ```bash
   npm install -g @openai/codex@latest
   ```

4. Verify in WSL:

   ```bash
   node -v
   npm -v
   codex --version
   ```

If `npm` works but `codex` does not, restart the WSL terminal so your npm global bin path is reloaded.

Tip: copy only commands from fenced code blocks. Do not paste markdown, git diffs, or `+`/`@@` patch lines into any shell.

As noted above, you should run Unix/Bash commands (including patch commands like `git apply <<'EOF' ... EOF`) inside your WSL/Linux shell, not in PowerShell. That heredoc-style syntax is for Bash, not PowerShell.

If you accidentally paste Unix/Bash syntax into PowerShell and your prompt changes to `>>`, PowerShell is waiting for more input. Press `Ctrl+C` to cancel, then switch back to your WSL terminal and run the intended command there.

Example (safe sequence):

```powershell
# PowerShell (Admin)
wsl --install
```

```bash
# WSL terminal (Ubuntu/Debian/etc.)
node -v
npm -v
npm install -g @openai/codex@latest
codex --version
```

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Team, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
