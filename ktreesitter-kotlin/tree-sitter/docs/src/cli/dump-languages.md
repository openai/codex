# `tree-sitter dump-languages`

The `dump-languages` command prints out a list of all the languages that the CLI knows about. This can be useful for debugging purposes, or for scripting. The paths to search comes from the config file's [`parser-directories`][parser-directories] object.

```bash
tree-sitter dump-languages [OPTIONS] # Aliases: langs
```

## Options

### `--config-path`

The path to the configuration file. Ordinarily, the CLI will use the default location as explained in the [init-config](./init-config.md) command. This flag allows you to explicitly override that default, and use a config defined elsewhere.

[parser-directories]: ./init-config.md#parser-directories
