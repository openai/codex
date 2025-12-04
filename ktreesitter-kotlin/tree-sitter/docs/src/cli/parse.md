# `tree-sitter parse`

The `parse` command parses source files using a Tree-sitter parser. You can pass any number of file paths and glob patterns
to `tree-sitter parse`, and it will parse all the given files. The command will exit with a non-zero status code if any
parse errors occurred.

```bash
tree-sitter parse [OPTIONS] [PATHS]... # Aliases: p
```

## Options

### `--paths <PATHS_FILE>`

The path to a file that contains paths to source files to parse.

### `--scope <SCOPE>`

The language scope to use for parsing. This is useful when the language is ambiguous.

### `-d/--debug`

Outputs parsing and lexing logs. This logs to stderr.

### `-0/--debug-build`

Compile the parser with debug flags enabled. This is useful when debugging issues that require a debugger like `gdb` or `lldb`.

### `-D/--debug-graph`

Outputs logs of the graphs of the stack and parse trees during parsing, as well as the actual parsing and lexing message.
The graphs are constructed with [graphviz dot][dot], and the output is written to `log.html`.

### `--wasm`

Compile and run the parser as a WASM module.

### `--dot`

Output the parse tree with [graphviz dot][dot].

### `-x/--xml`

Output the parse tree in XML format.

### `-c/--cst`

Output the parse tree in a pretty-printed CST format.

### `-s/--stat`

Show parsing statistics.

### `--timeout <TIMEOUT>`

Set the timeout for parsing a single file, in microseconds.

### `-t/--time`

Print the time taken to parse the file. If edits are provided, this will also print the time taken to parse the file after
each edit.

### `-q/--quiet`

Suppress main output.

### `--edits <EDITS>...`

Apply edits after parsing the file. Edits are in the form of `row,col|position delcount insert_text` where row and col, or position are 0-indexed.

### `--encoding <ENCODING>`

Set the encoding of the input file. By default, the CLI will look for the [`BOM`][bom] to determine if the file is encoded
in `UTF-16BE` or `UTF-16LE`. If no `BOM` is present, `UTF-8` is the default. One of `utf8`, `utf16-le`, `utf16-be`.

### `--open-log`

When using the `--debug-graph` option, open the log file in the default browser.

### `--config-path <CONFIG_PATH>`

The path to an alternative configuration (`config.json`) file. See [the init-config command](./init-config.md) for more information.

### `-n/--test-number <TEST_NUMBER>`

Parse a specific test in the corpus. The test number is the same number that appears in the output of `tree-sitter test`.

### `-r/--rebuild`

Force a rebuild of the parser before running tests.

### `--no-ranges`

Omit the node's ranges from the default parse output. This is useful when copying S-Expressions to a test file.

[dot]: https://graphviz.org/doc/info/lang.html
[bom]: https://en.wikipedia.org/wiki/Byte_order_mark
