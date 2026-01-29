# Codex UI (VS Code Extension)

Use Codex-style CLIs inside VS Code: sessions, chat, tools, diffs.

This extension:

- Starts / connects to a backend per workspace folder
- Manages sessions (create, switch, rename, hide)
- Renders chat output and tool activity (commands, file changes, diffs, approvals)

## Supported backends

You can choose a backend per session:

- **`codex`**: upstream Codex CLI (via `codex app-server`)
- **`codez`**: your Codez CLI (via `codez app-server`)
- **`opencode`**: OpenCode (via `opencode serve`)

Important notes:

- `codex` / `codez` sessions share the same protocol and can be reopened between each other.
- `opencode` uses a different history format/protocol. History is not shared with `codex`/`codez`, and sessions are kept separate.

## Prerequisites

This extension **does not bundle any CLI**. Install the backends you want to use and make sure they are available in your `PATH`:

- `codex` (for the `codex` backend)
- `codez` (for the `codez` backend)
- `opencode` (for the `opencode` backend)

Or set absolute paths via settings (see below).

## Usage

![screenshot](assets/image.png)

1. Open the Activity Bar view: **Codex UI**
2. Click **New** to create a session (you can pick `codex` / `codez` / `opencode`)
3. Type in the input box (Enter = send, Shift+Enter = newline)
4. Switch sessions from **Sessions** or the chat tab bar

## Feature notes

- **Rewind/Edit**: supported for `codez` and `opencode` sessions (not `codex`).
- **Accounts / login UI**: supported for `codez` sessions only (`opencode` does not use the same account flow).
- **OpenCode tool output**: rendered as Step/Tool cards optimized to avoid tool-spam.

## Settings

- `codez.cli.commands.codex`
  - Default: `codex`
  - If `codex` is not in your `PATH`, set an absolute path.
- `codez.cli.commands.codez`
  - Default: `codez`
  - If `codez` is not in your `PATH`, set an absolute path.
- `codez.backend.args`
  - Default: `["app-server"]`
  - Arguments passed to the `codex` / `codez` backend command.
- `codez.opencode.command`
  - Default: `opencode`
- `codez.opencode.args`
  - Default: `["serve"]`
  - Arguments passed to the `opencode` backend command.

## Development

1. Install dependencies

   ```bash
   pnpm install
   ```

2. (Re)generate protocol bindings (if missing / after protocol changes)

   ```bash
   cd ../codex-rs && cargo build -p codex-cli
   cd ../vscode-extension && pnpm run regen:protocol
   ```

3. Build

   ```bash
   pnpm run compile
   ```

4. Run in VS Code
   - Open this repo in VS Code
   - Run the debug configuration: **Run Extension (Codex UI)**

## Publishing (VS Code Marketplace)

1. Update `package.json` (`version`)
2. Package

   ```bash
   pnpm run vsix:package
   ```

3. Publish

   ```bash
   npx @vscode/vsce publish
   ```

## Specification

See `docs/spec.md`.

## Support

If you find this extension useful, you can support development via Buy Me a Coffee:

- https://buymeacoffee.com/harukary7518
