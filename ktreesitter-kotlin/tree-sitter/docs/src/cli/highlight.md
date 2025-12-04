# `tree-sitter highlight`

You can run syntax highlighting on an arbitrary file using `tree-sitter highlight`. This can either output colors directly
to your terminal using ANSI escape codes, or produce HTML (if the `--html` flag is passed). For more information, see
[the syntax highlighting page](../3-syntax-highlighting.md).

```bash
tree-sitter highlight [OPTIONS] [PATHS]... # Aliases: hi
```

## Options

### `-H/--html`

Output an HTML document with syntax highlighting.

### `--css-classes`

Output HTML with CSS classes instead of inline styles.

### `--check`

Check that the highlighting captures conform strictly to the standards.

### `--captures-path <CAPTURES_PATH>`

The path to a file with captures. These captures would be considered the "standard" captures to compare against.

### `--query-paths <QUERY_PATHS>`

The paths to query files to use for syntax highlighting. These should end in `highlights.scm`.

### `--scope <SCOPE>`

The language scope to use for syntax highlighting. This is useful when the language is ambiguous.

### `-t/--time`

Print the time taken to highlight the file.

### `-q/--quiet`

Suppress main output.

### `--paths <PATHS_FILE>`

The path to a file that contains paths to source files to highlight

### `--config-path <CONFIG_PATH>`

The path to an alternative configuration (`config.json`) file. See [the init-config command](./init-config.md) for more information.

### `-n/--test-number <TEST_NUMBER>`

Highlight the contents of a specific test.
