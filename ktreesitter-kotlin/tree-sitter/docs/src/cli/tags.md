# `tree-sitter tags`

You can run symbol tagging on an arbitrary file using `tree-sitter tags`. This will output a list of tags.
For more information, see [the code navigation page](../4-code-navigation.md#tagging-and-captures).

```bash
tree-sitter tags [OPTIONS] [PATHS]...
```

## Options

### `--scope <SCOPE>`

The language scope to use for symbol tagging. This is useful when the language is ambiguous.

### `-t/--time`

Print the time taken to generate tags for the file.

### `-q/--quiet`

Suppress main output.

### `--paths <PATHS_FILE>`

The path to a file that contains paths to source files to tag.

### `--config-path <CONFIG_PATH>`

The path to an alternative configuration (`config.json`) file. See [the init-config command](./init-config.md) for more information.

### `-n/--test-number <TEST_NUMBER>`

Generate tags from the contents of a specific test.
