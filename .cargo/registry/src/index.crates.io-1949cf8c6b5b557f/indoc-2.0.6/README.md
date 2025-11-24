Indented Documents (indoc)
==========================

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/indoc-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/indoc)
[<img alt="crates.io" src="https://img.shields.io/crates/v/indoc.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/indoc)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-indoc-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/indoc)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/indoc/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/indoc/actions?query=branch%3Amaster)

This crate provides a procedural macro for indented string literals. The
`indoc!()` macro takes a multiline string literal and un-indents it at compile
time so the leftmost non-space character is in the first column.

```toml
[dependencies]
indoc = "2"
```

*Compiler requirement: rustc 1.56 or greater.*

<br>

## Using indoc

```rust
use indoc::indoc;

fn main() {
    let testing = indoc! {"
        def hello():
            print('Hello, world!')

        hello()
    "};
    let expected = "def hello():\n    print('Hello, world!')\n\nhello()\n";
    assert_eq!(testing, expected);
}
```

Indoc also works with raw string literals:

```rust
use indoc::indoc;

fn main() {
    let testing = indoc! {r#"
        def hello():
            print("Hello, world!")

        hello()
    "#};
    let expected = "def hello():\n    print(\"Hello, world!\")\n\nhello()\n";
    assert_eq!(testing, expected);
}
```

And byte string literals:

```rust
use indoc::indoc;

fn main() {
    let testing = indoc! {b"
        def hello():
            print('Hello, world!')

        hello()
    "};
    let expected = b"def hello():\n    print('Hello, world!')\n\nhello()\n";
    assert_eq!(testing[..], expected[..]);
}
```

<br>

## Formatting macros

The indoc crate exports five additional macros to substitute conveniently for
the standard library's formatting macros:

- `formatdoc!($fmt, ...)`&ensp;&mdash;&ensp;equivalent to `format!(indoc!($fmt), ...)`
- `printdoc!($fmt, ...)`&ensp;&mdash;&ensp;equivalent to `print!(indoc!($fmt), ...)`
- `eprintdoc!($fmt, ...)`&ensp;&mdash;&ensp;equivalent to `eprint!(indoc!($fmt), ...)`
- `writedoc!($dest, $fmt, ...)`&ensp;&mdash;&ensp;equivalent to `write!($dest, indoc!($fmt), ...)`
- `concatdoc!(...)`&ensp;&mdash;&ensp;equivalent to `concat!(...)` with each string literal wrapped in `indoc!`

```rust
use indoc::{concatdoc, printdoc};

const HELP: &str = concatdoc! {"
    Usage: ", env!("CARGO_BIN_NAME"), " [options]

    Options:
        -h, --help
"};

fn main() {
    printdoc! {"
        GET {url}
        Accept: {mime}
        ",
        url = "http://localhost:8080",
        mime = "application/json",
    }
}
```

<br>

## Explanation

The following rules characterize the behavior of the `indoc!()` macro:

1. Count the leading spaces of each line, ignoring the first line and any lines
   that are empty or contain spaces only.
2. Take the minimum.
3. If the first line is empty i.e. the string begins with a newline, remove the
   first line.
4. Remove the computed number of spaces from the beginning of each line.

<br>

## Unindent

Indoc's indentation logic is available in the `unindent` crate. This may be
useful for processing strings that are not statically known at compile time.

The crate exposes two functions:

- `unindent(&str) -> String`
- `unindent_bytes(&[u8]) -> Vec<u8>`

```rust
use unindent::unindent;

fn main() {
    let indented = "
            line one
            line two";
    assert_eq!("line one\nline two", unindent(indented));
}
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
