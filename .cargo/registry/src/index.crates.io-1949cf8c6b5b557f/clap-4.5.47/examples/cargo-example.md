For more on creating a custom subcommand, see [the cargo
book](https://doc.rust-lang.org/cargo/reference/external-tools.html#custom-subcommands).
The crate [`clap-cargo`](https://github.com/crate-ci/clap-cargo) can help in
mimicking cargo's interface.

The help looks like:
```console
$ cargo-example --help
Usage: cargo <COMMAND>

Commands:
  example  A simple to use, efficient, and full-featured Command Line Argument Parser
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ cargo-example example --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: cargo example [OPTIONS]

Options:
      --manifest-path <PATH>  
  -h, --help                  Print help
  -V, --version               Print version

```

Then to directly invoke the command, run:
```console
$ cargo-example example
None

$ cargo-example example --manifest-path Cargo.toml
Some("Cargo.toml")

```
