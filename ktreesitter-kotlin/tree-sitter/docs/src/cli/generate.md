# `tree-sitter generate`

The most important command you'll use is `tree-sitter generate`. This command reads the `grammar.js` file in your current
working directory and creates a file called `src/parser.c`, which implements the parser. After making changes to your grammar,
just run `tree-sitter generate` again.

```bash
tree-sitter generate [OPTIONS] [GRAMMAR_PATH] # Aliases: gen, g
```

The grammar path argument allows you to specify a path to a `grammar.js` JavaScript file, or `grammar.json` JSON file.
In case your `grammar.js` file is in a non-standard path, you can specify it yourself. But, if you are using a parser
where `grammar.json` was already generated, or it was hand-written, you can tell the CLI to generate the parser *based*
on this JSON file. This avoids relying on a JavaScript file and avoids the dependency on a JavaScript runtime.

If there is an ambiguity or *local ambiguity* in your grammar, Tree-sitter will detect it during parser generation, and
it will exit with a `Unresolved conflict` error message. To learn more about conflicts and how to handle them, check out
the section on [`Structuring Rules Well`](../creating-parsers/3-writing-the-grammar.md#structuring-rules-well)
in the user guide.

## Options

### `-l/--log`

Print the log of the parser generation process. This is really only useful if you know what you're doing, or are investigating
a bug in the CLI itself. It logs info such as what tokens are included in the error recovery state,
what keywords were extracted, what states were split and why, and the entry point state.

### `--abi <VERSION>`

The ABI to use for parser generation. The default is ABI 15, with ABI 14 being a supported target.

### `-b/--build`

Compile all defined languages in the current directory. The cli will automatically compile the parsers after generation,
and place them in the cache dir.

### `-0/--debug-build`

Compile the parser with debug flags enabled. This is useful when debugging issues that require a debugger like `gdb` or `lldb`.

### `--libdir <PATH>`

The directory to place the compiled parser(s) in.
On Unix systems, the default path is `$XDG_CACHE_HOME/tree-sitter` if `$XDG_CACHE_HOME` is set,
otherwise `$HOME/.config/tree-sitter` is used. On Windows, the default path is `%LOCALAPPDATA%\tree-sitter` if available,
otherwise `$HOME\AppData\Local\tree-sitter` is used.

### `-o/--output`

The directory to place the generated parser in. The default is `src/` in the current directory.

### `--report-states-for-rule <RULE>`

Print the overview of states from the given rule. This is useful for debugging and understanding the generated parser's
item sets for all given states in a given rule. To solely view state count numbers for rules, pass in `-` for the rule argument.
To view the overview of states for every rule, pass in `*` for the rule argument.

### `--js-runtime <EXECUTABLE>`

The path to the JavaScript runtime executable to use when generating the parser. The default is `node`.
Note that you can also set this with `TREE_SITTER_JS_RUNTIME`.
