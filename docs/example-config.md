# Sample configuration

For a full sample configuration, see [this documentation](https://developers.openai.com/codex/config-sample).

## Keymap snippet

```toml
[tui.keymap]
preset = "latest" # currently points to v1 defaults

[tui.keymap.global]
open_transcript = "ctrl-t"
open_external_editor = "ctrl-g"

[tui.keymap.chat]
edit_previous_message = "esc"
confirm_edit_previous_message = "enter"

[tui.keymap.composer]
submit = "enter"
queue = "tab"
toggle_shortcuts = ["?", "shift-?"]

[tui.keymap.pager]
close = ["q", "ctrl-c"]
close_transcript = ["ctrl-t"]

[tui.keymap.list]
accept = "enter"
cancel = "esc"

[tui.keymap.approval]
approve = "y"
decline = ["esc", "n"]

[tui.keymap.onboarding]
quit = ["q", "ctrl-c", "ctrl-d"]
```

For a complete, commented template:
Keymap template: https://github.com/openai/codex/blob/main/docs/default-keymap.toml
For precedence, safety invariants, and testing notes, see `docs/tui-keymap.md`.
