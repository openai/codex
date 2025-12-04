# `tree-sitter playground`

The `playground` command allows you to start a local playground to test your parser interactively.

```bash
tree-sitter playground [OPTIONS] # Aliases: play, pg, web-ui
```

```admonish note
For this to work, you must have already built the parser as a WASM module. This can be done with the [`build`](./build.md) subcommand
(`tree-sitter build --wasm`).
```

## Options

### `-q/--quiet`

Don't automatically open the playground in the default browser.

### `--grammar-path <GRAMMAR_PATH>`

The path to the directory containing the grammar and wasm files.
