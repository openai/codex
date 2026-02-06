# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## TUI keymap

The TUI supports rebinding shortcuts via `[tui.keymap]` in `~/.codex/config.toml`.

Use this complete, commented defaults template.
Keymap template: https://github.com/openai/codex/blob/main/docs/default-keymap.toml
For implementation details, safety contracts, and testing notes, see `docs/tui-keymap.md`.

### Precedence

Precedence is applied in this order (highest first):

1. Context-specific binding (`[tui.keymap.<context>]`)
2. Global binding (`[tui.keymap.global]`) for chat/composer fallback actions
3. Built-in preset defaults (`preset`)

### Presets

- `latest`: moving alias for the newest preset; today `latest -> v1`
- `v1`: frozen legacy/current defaults

When defaults change in the future, a new version (for example `v2`) is added and
`latest` may move to it. Pin to `v1` if you want stable historical behavior.

### Supported actions

- `global`: `open_transcript`, `open_external_editor`, `edit_previous_message`,
  `confirm_edit_previous_message`, `submit`, `queue`, `toggle_shortcuts`
- `chat`: `edit_previous_message`, `confirm_edit_previous_message`
- `composer`: `submit`, `queue`, `toggle_shortcuts`
- `editor`: `insert_newline`, `move_left`, `move_right`, `move_up`, `move_down`,
  `move_word_left`, `move_word_right`, `move_line_start`, `move_line_end`,
  `delete_backward`, `delete_forward`, `delete_backward_word`, `delete_forward_word`,
  `kill_line_start`, `kill_line_end`, `yank`
- `pager`: `scroll_up`, `scroll_down`, `page_up`, `page_down`, `half_page_up`,
  `half_page_down`, `jump_top`, `jump_bottom`, `close`, `close_transcript`,
  `edit_previous_message`, `edit_next_message`, `confirm_edit_message`
- `list`: `move_up`, `move_down`, `accept`, `cancel`
- `approval`: `open_fullscreen`, `approve`, `approve_for_session`,
  `approve_for_prefix`, `decline`, `cancel`
- `onboarding`: `move_up`, `move_down`, `select_first`, `select_second`,
  `select_third`, `confirm`, `cancel`, `quit`, `toggle_animation`

For long-term behavior and evolution guidance, see `docs/tui-keymap.md`.
For a quick action inventory, see `docs/keymap-action-matrix.md`.
On onboarding API-key entry, printable `quit` bindings are treated as text input
once the field contains text; use control/alt chords for always-available quit
shortcuts.

### Key format

Use lowercase key identifiers with `-` separators, for example:

- `ctrl-a`
- `shift-enter`
- `alt-page-down`
- `?`

Actions accept a single key or multiple keys:

- `submit = "enter"`
- `submit = ["enter", "ctrl-j"]`
- `submit = []` (explicitly unbind)

Some defaults intentionally include multiple variants for one logical shortcut
because terminal modifier reporting can differ by platform/emulator. For
example, `?` may arrive as plain `?` or `shift-?`, and control chords may
arrive with or without `SHIFT`.

Aliases like `escape`, `pageup`, and `pgdn` are normalized.

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
