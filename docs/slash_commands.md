# Slash commands

For an overview of Codex CLI slash commands, see [this documentation](https://developers.openai.com/codex/cli/slash-commands).

## TUI

In the TUI, type `/` to open the slash-command popup. The popup uses the same command order as the
in-app `/help` page, with `/help` pinned at the top for discovery.

For commands that have both an interactive picker flow and a direct argument form, the bare
`/command` form opens the picker and `/command ...` runs the direct argument form instead. Use
`/help` inside the TUI for the current list of supported commands and argument syntax. Argument
parsing uses shell-style quoting, so quote values with spaces when needed.
