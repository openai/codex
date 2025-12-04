# Using Parsers

This guide covers the fundamental concepts of using Tree-sitter, which is applicable across all programming languages.
Although we'll explore some C-specific details that are valuable for direct C API usage or creating new language bindings,
the core concepts remain the same.

Tree-sitter's parsing functionality is implemented through its C API, with all functions documented in the [tree_sitter/api.h][api.h]
header file, but if you're working in another language, you can use one of the following bindings found [here](../index.md#language-bindings),
each providing idiomatic access to Tree-sitter's functionality. Of these bindings, the official ones have their own API docs
hosted online at the following pages:

- [Go][go]
- [Java]
- [JavaScript (Node.js)][javascript]
- [Kotlin][kotlin]
- [Python][python]
- [Rust][rust]
- [Zig][zig]

[api.h]: https://github.com/tree-sitter/tree-sitter/blob/master/lib/include/tree_sitter/api.h
[go]: https://pkg.go.dev/github.com/tree-sitter/go-tree-sitter
[java]: https://tree-sitter.github.io/java-tree-sitter
[javascript]: https://tree-sitter.github.io/node-tree-sitter
[kotlin]: https://tree-sitter.github.io/kotlin-tree-sitter
[python]: https://tree-sitter.github.io/py-tree-sitter
[rust]: https://docs.rs/tree-sitter
[zig]: https://tree-sitter.github.io/zig-tree-sitter
