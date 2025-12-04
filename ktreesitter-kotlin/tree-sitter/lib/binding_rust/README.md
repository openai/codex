# Rust Tree-sitter

[![crates.io badge]][crates.io]

[crates.io]: https://crates.io/crates/tree-sitter
[crates.io badge]: https://img.shields.io/crates/v/tree-sitter.svg?color=%23B48723

Rust bindings to the [Tree-sitter][] parsing library.

## Basic Usage

First, create a parser:

```rust
use tree_sitter::{InputEdit, Language, Parser, Point};

let mut parser = Parser::new();
```

Add the `cc` crate to your `Cargo.toml` under `[build-dependencies]`:

```toml
[build-dependencies]
cc="*"
```

Then, add a language as a dependency:

```toml
[dependencies]
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
```

To then use a language, you assign them to the parser.

```rust
parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("Error loading Rust grammar");
```

Now you can parse source code:

```rust
let source_code = "fn test() {}";
let mut tree = parser.parse(source_code, None).unwrap();
let root_node = tree.root_node();

assert_eq!(root_node.kind(), "source_file");
assert_eq!(root_node.start_position().column, 0);
assert_eq!(root_node.end_position().column, 12);
```

### Editing

Once you have a syntax tree, you can update it when your source code changes.
Passing in the previous edited tree makes `parse` run much more quickly:

```rust
let new_source_code = "fn test(a: u32) {}";

tree.edit(&InputEdit {
  start_byte: 8,
  old_end_byte: 8,
  new_end_byte: 14,
  start_position: Point::new(0, 8),
  old_end_position: Point::new(0, 8),
  new_end_position: Point::new(0, 14),
});

let new_tree = parser.parse(new_source_code, Some(&tree));
```

### Text Input

The source code to parse can be provided either as a string, a slice, a vector,
or as a function that returns a slice. The text can be encoded as either UTF8 or UTF16:

```rust
// Store some source code in an array of lines.
let lines = &[
    "pub fn foo() {",
    "  1",
    "}",
];

// Parse the source code using a custom callback. The callback is called
// with both a byte offset and a row/column offset.
let tree = parser.parse_with(&mut |_byte: usize, position: Point| -> &[u8] {
    let row = position.row as usize;
    let column = position.column as usize;
    if row < lines.len() {
        if column < lines[row].as_bytes().len() {
            &lines[row].as_bytes()[column..]
        } else {
            b"\n"
        }
    } else {
        &[]
    }
}, None).unwrap();

assert_eq!(
  tree.root_node().to_sexp(),
  "(source_file (function_item (visibility_modifier) (identifier) (parameters) (block (number_literal))))"
);
```

[tree-sitter]: https://github.com/tree-sitter/tree-sitter

## Features

- **std** - This feature is enabled by default and allows `tree-sitter` to use the standard library.
  - Error types implement the `std::error:Error` trait.
  - `regex` performance optimizations are enabled.
  - The DOT graph methods are enabled.
- **wasm** - This feature allows `tree-sitter` to be built for Wasm targets using the `wasmtime-c-api` crate.
