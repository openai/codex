For more on creating a custom subcommand, see [the cargo
book](https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands).
The crate [`clap-cargo`](https://github.com/crate-ci/clap-cargo) can help in
mimicking cargo's interface.

The help looks like:
```console
$ cargo-example-derive --help
Usage: cargo <COMMAND>

Commands:
  example-derive  A simple to use, efficient, and full-featured Command Line Argument Parser
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ cargo-example-derive example-derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: cargo example-derive [OPTIONS]

Options:
      --manifest-path <MANIFEST_PATH>  
  -h, --help                           Print help
  -V, --version                        Print version

```

Then to directly invoke the command, run:
```console
$ cargo-example-derive example-derive
None

$ cargo-example-derive example-derive --manifest-path Cargo.toml
Some("Cargo.toml")

```
