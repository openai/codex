# Codex UI VS Code Extension - Spec (Implementation-aligned)

This document describes the current behavior of the extension (what it does today), not an aspirational design.

## 1. Terminology

- **Backend**: Codex CLI running `app-server`.
- **Session**: A chat session tracked by the extension (mapped to a backend thread).
- **Thread**: Backend concept (thread id, persisted by Codex CLI).
- **Block**: A rendered item in the chat log (user/assistant/reasoning/tool/etc).

## 2. Views

The extension contributes a **Codex UI** activity bar container with:

- **Sessions**: A tree view listing sessions per workspace folder.
- **Chat**: A WebviewView that shows chat logs and an input box.

## 3. Backend lifecycle

- The backend command defaults to `codex` and args default to `["app-server"]`.
- The extension can start a backend per workspace folder (keyed by the workspace folder URI).
- If a backend is already running for a workspace folder, the extension reuses it.
- The extension avoids noisy notifications for backend start/reuse.

## 4. Sessions

### 4.1 Create

- **New** creates a new backend thread and a new extension session for the currently selected workspace folder.

### 4.2 Switch

- Sessions can be switched from:
  - The **Sessions** tree view
  - The **Chat** tab bar

### 4.3 Rename

- A session can be renamed via a context menu.
- Rename updates both:
  - the chat tab label
  - the Sessions list label

### 4.4 Hide

- There are two distinct behaviors:

1. **Hide Tab** (Chat tab bar)

- Hides the session tab from the Chat tab bar.
- The session remains visible in Sessions and can be re-opened.

2. **Close Session (Hide from Sessions)** (Sessions view)

- Hides the session from the Sessions list.
- The underlying Codex CLI log files are NOT deleted.

## 5. Chat input

- Enter sends the message.
- Shift+Enter inserts a newline.
- The input box remains enabled while the model is responding (so the user can type the next message).
- The send action is guarded so it cannot be triggered without an active session.

### 5.1 Input history

- Sent messages are stored in an in-memory history.
- Up/Down arrow cycles through the history when the cursor is at the start/end of the input (implementation detail).

## 6. Slash commands

Supported commands (handled by the extension):

- `/new` - create a new session
- `/diff` - open latest diff
- `/rename <title>` - rename session
- `/help` - show help

Custom prompts:

- Loaded from `$CODEX_HOME/prompts` (defaults to `~/.codex/prompts`)
- Only `.md` files are loaded
- If the backend sends `list_custom_prompts_response`, the list is replaced with that payload
- Commands are invoked as `/prompts:<prompt-name>`
- The prompt body is expanded before sending (supports `$1..$9`, `$ARGUMENTS`, and `$NAME` placeholders)
- Prompts are reloaded when a session is created or selected in the extension
- Prompt arguments are parsed with `shell-quote` (shlex-like quoting support)

Unknown commands are passed through to the backend (no local error).
Custom prompts only execute via `/prompts:<name>`; UI commands (`/new`, `/diff`, `/rename`, `/help`)
keep their local behavior even if a prompt shares the same name.

## 7. Mentions


Mentions follow a CLI-like rule: `@...` must start on a whitespace boundary.

### 7.1 `@selection`

- `@selection` is expanded to a **file reference** (path + line range), not file contents.
- The expansion format is:
  - `@relative/path#L<start>-L<end>` (or `#L<line>` for a single line)

### 7.2 `@relative/path`

- `@relative/path` sends the path as-is.
- The extension validates that the target exists and is a file.
- Absolute paths and paths containing `:` are rejected.

### 7.3 Legacy: `@file:relative/path`

- Treated as an alias of `@relative/path` (the `file:` prefix is removed).

## 8. Suggestions (autocomplete)

The webview provides in-input suggestions for:

- Slash commands, in this order:
  1) Custom prompts from `$CODEX_HOME/prompts`
  2) UI commands (`/new`, `/diff`, `/rename`, `/help`)
- Mentions (`@selection`, file paths)

### 8.1 File suggestions

- File suggestions are backed by a file index provided by the extension.
- The index is per active session/workspace.
- The index is capped (to avoid huge workspaces).

### 8.2 Keyboard behavior

- ArrowUp/ArrowDown changes the selected suggestion.
- The list auto-scrolls to keep the active item visible.
- Accepting a suggestion closes the suggestion UI (so the next Enter sends).

## 9. Rendering

### 9.1 Markdown

- Assistant/user/reasoning bodies are rendered via `markdown-it`.
- Links:
  - External links open via `openExternal`.
  - Workspace-relative links (e.g. `README.md`, `./docs/spec.md`, `/README.md`) open as files.
  - `#L10` / `#L10C5` fragments jump to the corresponding position.

### 9.2 Ctrl/Cmd-click file links

- A file-like string rendered as a link triggers `openFile`.
- The extension resolves workspace-relative paths against the active session workspace root.

## 10. Persistence

### 10.1 Sessions

Sessions are persisted in workspace state:

- `id`, `backendKey`, `workspaceFolderUri`, `title`, `threadId`

### 10.2 Chat runtime

Per session, the extension persists runtime state:

- `blocks` (chat history)
- `latestDiff`
- `statusText`

Persistence is done on a small debounce to avoid excessive writes.

### 10.3 Webview UI state

The webview stores UI state with `vscode.setState()`:

- `detailsState` (open/close state of details)

## 11. Status text

The status line (below the composer) shows:

- `ctx remaining=<percent> (<used>/<max>)`
- `worked=<seconds>s`

Token breakdown is intentionally not shown.

## 12. Event handling

The backend emits a mixture of v2 and legacy events.

### 12.1 Global notifications

Example: `thread/started`

- Rendered as an **info notice** (white card), not as a debug dump.
- Dedupe: for the same workspace folder (`cwd`), only one notice is kept.
- Display fields:
  - Working directory
  - CLI version
  - Git origin

### 12.2 Turn/Item events

- User/assistant messages are shown as chat blocks.
- Tool events are shown as tool blocks (command, file changes, web search, etc).

### 12.3 Web search

- Shown as a single-line card:
  - `🔎 <query>`
- Begin/end events are merged by `call_id` when possible.

### 12.4 File changes / patch apply

- Shown as a **Changes** block.
- Can include per-file collapsible diffs when `latestDiff` is available.

### 12.5 Command execution

- Shown as a **Command** block.
- If the command contains a common shell wrapper (e.g. `/bin/zsh -lc cd <cwd> && ...`), the UI displays only the actual command portion.

### 12.6 Plan updates

- Plan can be shown as a dedicated block.
- Statuses are represented compactly (icons), without adding extra lines.

### 12.7 Unknown/unhandled events

- Unknown events are collected into a debug block ("Other events (debug)") so they remain inspectable.

## 13. Colors

Card colors are grouped by message kind:

- `user`: neutral background with a blue-ish border (distinct from web search)
- `assistant`: neutral
- `reasoning`: green-ish
- `system`: yellow-ish
- `error`: neutral with error emphasis
- `tool:command`: purple-ish
- `tool:changes`: orange-ish
- `tool:webSearch`: light blue
- `tool:mcp`: cyan-ish

## 14. Known limitations

- File indexing is capped; extremely large workspaces may not list every file.
- `@relative/path` only supports workspace-relative file paths.

## 15. Debugging

If **New** / **Send** appears unresponsive:

- The webview JS may have crashed.
- Check the Output channel: **Codex UI**.
