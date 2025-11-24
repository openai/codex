# Tree-sitter Highlight

[![crates.io badge]][crates.io]

[crates.io]: https://crates.io/crates/tree-sitter-highlight
[crates.io badge]: https://img.shields.io/crates/v/tree-sitter-highlight.svg?color=%23B48723

## Usage

Add this crate, and the language-specific crates for whichever languages you want
to parse, to your `Cargo.toml`:

```toml
[dependencies]
tree-sitter-highlight = "0.22.0"
tree-sitter-javascript = "0.21.3"
```

Define the list of highlight names that you will recognize:

```rust
let highlight_names = [
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "embedded",
    "function",
    "function.builtin",
    "keyword",
    "module",
    "number",
    "operator",
    "property",
    "property.builtin",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];
```

Create a highlighter. You need one of these for each thread that you're using for
syntax highlighting:

```rust
use tree_sitter_highlight::Highlighter;

let mut highlighter = Highlighter::new();
```

Load some highlighting queries from the `queries` directory of the language repository:

```rust
use tree_sitter_highlight::HighlightConfiguration;

let javascript_language = tree_sitter_javascript::language();

let mut javascript_config = HighlightConfiguration::new(
    javascript_language,
    "javascript",
    tree_sitter_javascript::HIGHLIGHT_QUERY,
    tree_sitter_javascript::INJECTIONS_QUERY,
    tree_sitter_javascript::LOCALS_QUERY,
).unwrap();
```

Configure the recognized names:

```rust
javascript_config.configure(&highlight_names);
```

Highlight some code:

```rust
use tree_sitter_highlight::HighlightEvent;

let highlights = highlighter.highlight(
    &javascript_config,
    b"const x = new Y();",
    None,
    |_| None
).unwrap();

for event in highlights {
    match event.unwrap() {
        HighlightEvent::Source {start, end} => {
            eprintln!("source: {start}-{end}");
        },
        HighlightEvent::HighlightStart(s) => {
            eprintln!("highlight style started: {s:?}");
        },
        HighlightEvent::HighlightEnd => {
            eprintln!("highlight style ended");
        },
    }
}
```

The last parameter to `highlight` is a _language injection_ callback. This allows
other languages to be retrieved when Tree-sitter detects an embedded document
(for example, a piece of JavaScript code inside a `script` tag within HTML).
