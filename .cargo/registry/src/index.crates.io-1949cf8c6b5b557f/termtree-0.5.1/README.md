# termtree [![Main](https://github.com/rust-cli/termtree/actions/workflows/main.yml/badge.svg)](https://github.com/rust-cli/termtree/actions/workflows/main.yml)

> Visualize tree-like data on the command-line

[API documentation](https://docs.rs/termtree)

## Example

An example program is provided under the "examples" directory to mimic the `tree(1)`
linux program

```bash
$ cargo run --example tree target
    Finished debug [unoptimized + debuginfo] target(s) in 0.0 secs
     Running `target/debug/examples/tree target`
target
└── debug
    ├── .cargo-lock
    ├── .fingerprint
    |   └── termtree-21a5bdbd42e0b6da
    |       ├── dep-example-tree
    |       ├── dep-lib-termtree
    |       ├── example-tree
    |       ├── example-tree.json
    |       ├── lib-termtree
    |       └── lib-termtree.json
    ├── build
    ├── deps
    |   └── libtermtree.rlib
    ├── examples
    |   ├── tree
    |   └── tree.dSYM
    |       └── Contents
    |           ├── Info.plist
    |           └── Resources
    |               └── DWARF
    |                   └── tree
    ├── libtermtree.rlib
    └── native
```

## Related Crates

- [`treeline`](https://crates.io/crates/treeline): termtree was forked from this.
- [`tree_decorator`](https://crates.io/crates/tree_decorator)
- [`xtree`](https://crates.io/crates/xtree)
- [`ptree`](https://crates.io/crates/ptree)

## License

Licensed under MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
