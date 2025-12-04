# `tree-sitter test`

The `test` command is used to run the test suite for a parser.

```bash
tree-sitter test [OPTIONS] # Aliases: t
```

## Options

### `-i/--include <INCLUDE>`

Only run tests whose names match this regex.

### `-e/--exclude <EXCLUDE>`

Skip tests whose names match this regex.

### `-u/--update`

Update the expected output of tests.

```admonish info
Tests containing `ERROR` nodes or `MISSING` nodes will not be updated.
```

### `-d/--debug`

Outputs parsing and lexing logs. This logs to stderr.

### `-0/--debug-build`

Compile the parser with debug flags enabled. This is useful when debugging issues that require a debugger like `gdb` or `lldb`.

### `-D/--debug-graph`

Outputs logs of the graphs of the stack and parse trees during parsing, as well as the actual parsing and lexing message.
The graphs are constructed with [graphviz dot][dot], and the output is written to `log.html`.

### `--wasm`

Compile and run the parser as a WASM module.

### `--open-log`

When using the `--debug-graph` option, open the log file in the default browser.

### `--config-path <CONFIG_PATH>`

The path to an alternative configuration (`config.json`) file. See [the init-config command](./init-config.md) for more information.

### `--show-fields`

Force showing fields in test diffs.

### `--stat <STAT>`

Show parsing statistics when tests are being run. One of `all`, `outliers-and-total`, or `total-only`.

- `all`: Show statistics for every test.

- `outliers-and-total`: Show statistics only for outliers, and total statistics.

- `total-only`: Show only total statistics.

### `-r/--rebuild`

Force a rebuild of the parser before running tests.

### `--overview-only`

Only show the overview of the test results, and not the diff.
