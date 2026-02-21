# TUI Keymap Action Matrix

This file defines the supported keymap actions and their default `v1` bindings.
For runtime behavior, safety invariants, and testing guidance, see
`docs/tui-keymap.md`.

## Preset behavior

- `latest` is an alias to the newest shipped preset.
- Today, `latest -> v1`.
- To keep stable behavior over time, pin `preset = "v1"`.

## Precedence

1. `tui.keymap.<context>.<action>`
2. `tui.keymap.global.<action>` (chat/composer fallback actions only)
3. Preset default (`v1` today)

## Default `v1` Compatibility Notes

- Some actions intentionally ship with multiple bindings for the same logical
  shortcut because terminals differ in modifier reporting.
- Today this includes:
  - `composer.toggle_shortcuts`: `?` and `shift-?`
  - `approval.open_fullscreen`: `ctrl-a` and `ctrl-shift-a`
  - `onboarding.toggle_animation`: `ctrl-.` and `ctrl-shift-.`
- Shifted letter bindings are also matched compatibly across terminal variants
  when reported as uppercase letters without explicit `SHIFT` (for example
  `shift-i` matching `I`, `shift-a` matching `A`, and `shift-o` matching `O`).
- Keep these paired defaults unless/until key-event normalization is made
  platform-consistent at a lower layer.

## Action Definitions

### `global`

- `open_transcript`: open transcript overlay
- `open_external_editor`: open external editor for current draft
- `edit_previous_message`: begin/advance edit-previous flow when composer is empty
- `confirm_edit_previous_message`: confirm selected previous message for editing
- `submit`: submit current draft
- `queue`: queue current draft while a task is running
- `toggle_shortcuts`: toggle composer shortcut overlay
- `toggle_vim_mode`: toggle Vim mode for composer input

### `chat`

- `edit_previous_message`: chat override for edit-previous flow
- `confirm_edit_previous_message`: chat override for edit confirmation

### `composer`

- `submit`: composer override for submit
- `queue`: composer override for queue
- `toggle_shortcuts`: composer override for shortcut overlay toggle

### `editor`

- `insert_newline`: insert newline in text editor
- `move_left` / `move_right` / `move_up` / `move_down`: cursor movement
- `move_word_left` / `move_word_right`: word movement
- `move_line_start` / `move_line_end`: line boundary movement
- `delete_backward` / `delete_forward`: single-char deletion
- `delete_backward_word` / `delete_forward_word`: word deletion
- `kill_line_start` / `kill_line_end`: kill to line boundary
- `yank`: paste kill-buffer contents

### `vim_normal`

- `enter_insert`: switch to insert mode
- `append_after_cursor`: move right and switch to insert mode
- `append_line_end`: move to end of line and switch to insert mode
- `insert_line_start`: move to beginning of line and switch to insert mode
- `open_line_below` / `open_line_above`: open line and switch to insert mode
- `move_left` / `move_right` / `move_up` / `move_down`: cursor movement
- `move_word_forward` / `move_word_backward` / `move_word_end`: word motions
- `move_line_start` / `move_line_end`: line boundary motions
- `delete_char`: delete character at cursor
- `delete_to_line_end`: delete from cursor to line end
- `yank_line`: copy current line
- `paste_after`: paste after cursor
- `start_delete_operator` / `start_yank_operator`: enter operator-pending state
- `cancel_operator`: clear pending operator

### `vim_operator`

- `delete_line`: apply delete operator to full line
- `yank_line`: apply yank operator to full line
- `motion_left` / `motion_right` / `motion_up` / `motion_down`: motion targets
- `motion_word_forward` / `motion_word_backward` / `motion_word_end`: word targets
- `motion_line_start` / `motion_line_end`: line targets
- `cancel`: cancel pending operator

### `pager`

- `scroll_up` / `scroll_down`: row scroll
- `page_up` / `page_down`: page scroll
- `half_page_up` / `half_page_down`: half-page scroll
- `jump_top` / `jump_bottom`: jump to top/bottom
- `close`: close pager overlay
- `close_transcript`: close transcript via transcript toggle binding
- `edit_previous_message` / `edit_next_message`: backtrack navigation in transcript
- `confirm_edit_message`: confirm selected backtrack message

### `list`

- `move_up` / `move_down`: list navigation
- `accept`: select current item
- `cancel`: close list view

### `approval`

- `open_fullscreen`: open full-screen approval details
- `approve`: approve primary request
- `approve_for_session`: approve-for-session option
- `approve_for_prefix`: approve-for-prefix option
- `decline`: decline request
- `cancel`: cancel elicitation request
- MCP elicitation safety rule: `Esc` is always treated as `cancel` (never
  `decline`) so dismissal cannot accidentally continue execution.

### `onboarding`

- `move_up` / `move_down`: onboarding list navigation
- `select_first` / `select_second` / `select_third`: numeric selection shortcuts
- `confirm`: confirm highlighted onboarding selection
- `cancel`: cancel current onboarding sub-flow
- `quit`: quit onboarding flow
- `toggle_animation`: switch welcome animation variant
- API-key entry guard: printable `quit` bindings are treated as text input once
  the API-key field has text; control/alt quit chords are not suppressed.
