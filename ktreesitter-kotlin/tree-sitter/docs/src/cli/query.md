# `tree-sitter query`

The `query` command is used to run a query on a parser, and view the results.

```bash
tree-sitter query [OPTIONS] <QUERY_PATH> [PATHS]... # Aliases: q
```

## Options

### `-t/--time`

Print the time taken to execute the query on the file.

### `-q/--quiet`

Suppress main output.

### `--paths <PATHS_FILE>`

The path to a file that contains paths to source files in which the query will be executed.

### `--byte-range <BYTE_RANGE>`

The range of byte offsets in which the query will be executed. The format is `start_byte:end_byte`.

### `--row-range <ROW_RANGE>`

The range of rows in which the query will be executed. The format is `start_row:end_row`.

### `--scope <SCOPE>`

The language scope to use for parsing and querying. This is useful when the language is ambiguous.

### `-c/--captures`

Order the query results by captures instead of matches.

### `--test`

Whether to run query tests or not.

### `--config-path <CONFIG_PATH>`

The path to an alternative configuration (`config.json`) file. See [the init-config command](./init-config.md) for more information.

### `-n/--test-number <TEST_NUMBER>`

Query the contents of a specific test.
