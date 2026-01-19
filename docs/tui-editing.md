# TUI editing (Reedline)

Codex’s TUIs use a shared textarea implementation for the prompt composer.

Two beta feature flags add more shell-like line editing:

- `tui_vi_mode`: Vi keybindings in the prompt composer.
- `tui_command_history`: Ctrl-R history search.

## Enable

Enable from inside the TUI via `/experimental`, or in `config.toml`:

```toml
[features]
tui_vi_mode = true
tui_command_history = true
```

## Vi mode

When `tui_vi_mode` is enabled:

- `Esc` switches between insert and normal mode.
- The footer indicator shows `vi: insert` / `vi: normal`.
- Popups (slash/file/skill) are only shown in insert mode.

To avoid ambiguity, Codex’s Esc-based UI behavior only runs in vi *normal* mode:

- While a task is running, `Esc` interrupts the task (if you are in insert mode, press `Esc`
  once to enter normal mode first).
- When the composer is empty and no popups/modals are open, `Esc` `Esc` enters backtrack (edit a
  previous message). If you are in insert mode, press `Esc` once first.

## Command history search

When `tui_command_history` is enabled:

- `Ctrl+R` opens a searchable history picker.
- `Enter` pastes the selected entry into the composer.
- `Esc` closes the picker.

The picker is backed by `$CODEX_HOME/history.jsonl` (JSON lines). Writing to that file is
controlled by the existing `[history]` config (for example, `persistence = "none"` disables
writing).
