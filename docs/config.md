# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## TUI behavior

Use `[tui]` to configure startup behavior for the terminal UI.

```toml
[tui]
# Start the composer in Vim "Normal" mode on launch.
vim_mode_default = true
```

`vim_mode_default` defaults to `false` (composer starts in insert mode).

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

- `latest`: moving alias for the newest preset; today `latest -> v3`
- `v3`: current defaults, exposing `copy = "ctrl-o"` in the keymap
- `v2`: restores `alt-d` as a `delete_forward_word` alias
- `v1`: frozen initial configurable keymap defaults

When defaults change in the future, a new version (for example `v4`) is added and
`latest` may move to it. Pin to `v1` if you want stable historical behavior.

TODO(docs): mirror this preset migration note on developers.openai.com.

### Supported actions

- `global`: `open_transcript`, `open_external_editor`, `copy`, `edit_previous_message`,
  `confirm_edit_previous_message`, `submit`, `queue`, `toggle_shortcuts`,
  `toggle_vim_mode`
- `chat`: `edit_previous_message`, `confirm_edit_previous_message`
- `composer`: `submit`, `queue`, `toggle_shortcuts`
- `editor`: `insert_newline`, `move_left`, `move_right`, `move_up`, `move_down`,
  `move_word_left`, `move_word_right`, `move_line_start`, `move_line_end`,
  `delete_backward`, `delete_forward`, `delete_backward_word`, `delete_forward_word`,
  `kill_line_start`, `kill_line_end`, `yank`
- `vim_normal`: `enter_insert`, `append_after_cursor`, `append_line_end`,
  `insert_line_start`, `open_line_below`, `open_line_above`, `move_left`,
  `move_right`, `move_up`, `move_down`, `move_word_forward`,
  `move_word_backward`, `move_word_end`, `move_line_start`, `move_line_end`,
  `delete_char`, `delete_to_line_end`, `yank_line`, `paste_after`,
  `start_delete_operator`, `start_yank_operator`, `cancel_operator`
- `vim_operator`: `delete_line`, `yank_line`, `motion_left`, `motion_right`,
  `motion_up`, `motion_down`, `motion_word_forward`, `motion_word_backward`,
  `motion_word_end`, `motion_line_start`, `motion_line_end`, `cancel`
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
arrive with or without `SHIFT`. Shifted letter bindings are also matched
compatibly when terminals report uppercase letters without explicit `SHIFT`
(for example, `shift-i` matching `I`).

Aliases like `escape`, `pageup`, and `pgdn` are normalized.

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## MCP tool approvals

Codex stores per-tool approval overrides for custom MCP servers under
`mcp_servers` in `~/.codex/config.toml`:

```toml
[mcp_servers.docs.tools.search]
approval_mode = "approve"
```

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

When Codex knows which client started the turn, the legacy notify JSON payload also includes a top-level `client` field. The TUI reports `codex-tui`, and the app server reports the `clientInfo.name` value from `initialize`.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## SQLite State DB

Codex stores the SQLite-backed state DB under `sqlite_home` (config key) or the
`CODEX_SQLITE_HOME` environment variable. When unset, WorkspaceWrite sandbox
sessions default to a temp directory; other modes default to `CODEX_HOME`.

## Custom CA Certificates

Codex can trust a custom root CA bundle for outbound HTTPS and secure websocket
connections when enterprise proxies or gateways intercept TLS. This applies to
login flows and to Codex's other external connections, including Codex
components that build reqwest clients or secure websocket clients through the
shared `codex-client` CA-loading path and remote MCP connections that use it.

Set `CODEX_CA_CERTIFICATE` to the path of a PEM file containing one or more
certificate blocks to use a Codex-specific CA bundle. If
`CODEX_CA_CERTIFICATE` is unset, Codex falls back to `SSL_CERT_FILE`. If
neither variable is set, Codex uses the system root certificates.

`CODEX_CA_CERTIFICATE` takes precedence over `SSL_CERT_FILE`. Empty values are
treated as unset.

The PEM file may contain multiple certificates. Codex also tolerates OpenSSL
`TRUSTED CERTIFICATE` labels and ignores well-formed `X509 CRL` sections in the
same bundle. If the file is empty, unreadable, or malformed, the affected Codex
HTTP or secure websocket connection reports a user-facing error that points
back to these environment variables.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

## Plan mode defaults

`plan_mode_reasoning_effort` lets you set a Plan-mode-specific default reasoning
effort override. When unset, Plan mode uses the built-in Plan preset default
(currently `medium`). When explicitly set (including `none`), it overrides the
Plan preset. The string value `none` means "no reasoning" (an explicit Plan
override), not "inherit the global default". There is currently no separate
config value for "follow the global default in Plan mode".

## Realtime start instructions

`experimental_realtime_start_instructions` lets you replace the built-in
developer message Codex inserts when realtime becomes active. It only affects
the realtime start message in prompt history and does not change websocket
backend prompt settings or the realtime end/inactive message.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
