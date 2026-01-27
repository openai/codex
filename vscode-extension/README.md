# Codex UI (VS Code Extension)

Use **Codex CLI** (via `codex app-server`) inside VS Code.

This extension:

- Starts / connects to the Codex CLI app-server per workspace folder
- Manages sessions (create, switch, rename, hide)
- Shows chat output including tool events (commands, file changes, diffs, approvals)

## Prerequisites (Codex CLI)

This extension **does not bundle Codex CLI**.

- Install Codex CLI separately and make sure `codex` is available in your `PATH`.
- Or set the full path via settings (`codez.backend.command`).

## Usage

![screenshot](assets/image.png)

1. Open the Activity Bar view: **Codex UI**
2. Click **New** to create a session
3. Type in the input box (Enter = send, Shift+Enter = newline)
4. Switch sessions from **Sessions** or the chat tab bar

### Settings

- `codez.backend.command`
  - Default: `codex`
  - If `codex` is not in your `PATH`, set an absolute path.
- `codez.backend.args`
  - Default: `["app-server"]`
- `codez.backend.kind`
  - Default: `app-server`
  - `app-server`: spawn Codex CLI (`codex app-server`) via JSON-RPC (existing behavior)
  - `opencode`: spawn `opencode serve` and talk to its HTTP API
- `codez.opencode.command`
  - Default: `opencode`
- `codez.opencode.args`
  - Default: `["serve"]`

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
