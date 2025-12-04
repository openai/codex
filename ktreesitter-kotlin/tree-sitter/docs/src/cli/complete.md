# `tree-sitter complete`

The `complete` command generates a completion script for your shell.
This script can be used to enable autocompletion for the `tree-sitter` CLI.

```bash
tree-sitter complete --shell <SHELL> # Aliases: comp
```

## Options

### `--shell <SHELL>`

The shell for which to generate the completion script.

Supported values: `bash`, `elvish`, `fish`, `power-shell`, `zsh`, and `nushell`.
