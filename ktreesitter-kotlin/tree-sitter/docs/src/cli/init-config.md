# `tree-sitter init-config`

This command initializes a configuration file for the Tree-sitter CLI.

```bash
tree-sitter init-config
```

These directories are created in the "default" location for your platform:

* On Unix, `$XDG_CONFIG_HOME/tree-sitter` or `$HOME/.config/tree-sitter`
* On Windows, `%APPDATA%\tree-sitter` or `$HOME\AppData\Roaming\tree-sitter`

```admonish info
The CLI will work if there's no config file present, falling back on default values for each configuration option.
```

When you run the `init-config` command, it will print out the location of the file that it creates so that you can easily
find and modify it.

The configuration file is a JSON file that contains the following fields:

## `parser-directories`

The [`tree-sitter highlight`](./highlight.md) command takes one or more file paths, and tries to automatically determine,
which language should be used to highlight those files. To do this, it needs to know *where* to look for Tree-sitter grammars
on your filesystem. You can control this using the `"parser-directories"` key in your configuration file:

```json
{
  "parser-directories": [
    "/Users/my-name/code",
    "/Users/my-name/other-code"
  ]
}
```

Any folder within one of these *parser directories* whose name begins with `tree-sitter-` will be treated as a Tree-sitter
grammar repository.

## `theme`

The [Tree-sitter highlighting system](../3-syntax-highlighting.md) works by annotating ranges of source code with logical
"highlight names" like `function.method`, `type.builtin`, `keyword`, etc. To decide what *color* should be used for rendering
each highlight, a *theme* is needed.

In your config file, the `"theme"` value is an object whose keys are dot-separated highlight names like
`function.builtin` or `keyword`, and whose values are JSON expressions that represent text styling parameters.

### Highlight Names

A theme can contain multiple keys that share a common subsequence. Examples:

* `variable` and `variable.parameter`
* `function`, `function.builtin`, and `function.method`

For a given highlight produced, styling will be determined based on the **longest matching theme key**. For example, the
highlight `function.builtin.static` would match the key `function.builtin` rather than `function`.

### Styling Values

Styling values can be any of the following:

* Integers from 0 to 255, representing ANSI terminal color ids.
* Strings like `"#e45649"` representing hexadecimal RGB colors.
* Strings naming basic ANSI colors like `"red"`, `"black"`, `"purple"`, or `"cyan"`.
* Objects with the following keys:
  * `color` — An integer or string as described above.
  * `underline` — A boolean indicating whether the text should be underlined.
  * `italic` — A boolean indicating whether the text should be italicized.
  * `bold` — A boolean indicating whether the text should be bold-face.

An example theme can be seen below:

```json
{
  "function": 26,
  "operator": {
    "bold": true,
    "color": 239
  },
  "variable.builtin": {
    "bold": true
  },
  "variable.parameter": {
    "underline": true
  },
  "type.builtin": {
    "color": 23,
    "bold": true
  },
  "keyword": 56,
  "type": 23,
  "number": {
    "bold": true,
    "color": 94
  },
  "constant": 94,
  "attribute": {
    "color": 124,
    "italic": true
  },
  "comment": {
    "color": 245,
    "italic": true
  },
  "constant.builtin": {
    "color": 94,
    "bold": true
  },
}
```

## `parse-theme`

The [`tree-sitter parse`](./parse.md) command will output a pretty-printed CST when the `-c/--cst` option is used. You can
control what colors are used for various parts of the tree in your configuration file.

```admonish note
Omitting a field will cause the relevant text to be rendered with its default color.
```

An example parse theme can be seen below:

```json
{
  "parse-theme": {
    // The color of node kinds
    "node-kind": [20, 20, 20],
    // The color of text associated with a node
    "node-text": [255, 255, 255],
    // The color of node fields
    "field": [42, 42, 42],
    // The color of the range information for unnamed nodes
    "row-color": [255, 255, 255],
    // The color of the range information for named nodes
    "row-color-named": [255, 130, 0],
    // The color of extra nodes
    "extra": [255, 0, 255],
    // The color of ERROR nodes
    "error": [255, 0, 0],
    // The color of MISSING nodes and their associated text
    "missing": [153, 75, 0],
    // The color of newline characters
    "line-feed": [150, 150, 150],
    // The color of backtick characters
    "backtick": [0, 200, 0],
    // The color of literals
    "literal": [0, 0, 200],
  }
}
```
