# `tree-sitter fuzz`

The `fuzz` command is used to fuzz a parser by performing random edits and ensuring that undoing these edits results in
consistent parse trees. It will fail if the parse trees are not equal, or if the changed ranges are inconsistent.

```bash
tree-sitter fuzz [OPTIONS] # Aliases: f
```

## Options

### `-s/--skip <SKIP>`

A list of test names to skip fuzzing.

### `--subdir <SUBDIR>`

The directory containing the parser. This is primarily useful in multi-language repositories.

### `--edits <EDITS>`

The maximum number of edits to perform. The default is 3.

### `--iterations <ITERATIONS>`

The number of iterations to run. The default is 10.

### `-i/--include <INCLUDE>`

Only run tests whose names match this regex.

### `-e/--exclude <EXCLUDE>`

Skip tests whose names match this regex.

### `--log-graphs`

Outputs logs of the graphs of the stack and parse trees during parsing, as well as the actual parsing and lexing message.
The graphs are constructed with [graphviz dot][dot], and the output is written to `log.html`.

### `-l/--log`

Outputs parsing and lexing logs. This logs to stderr.

### `-r/--rebuild`

Force a rebuild of the parser before running the fuzzer.

[dot]: https://graphviz.org/doc/info/lang.html
