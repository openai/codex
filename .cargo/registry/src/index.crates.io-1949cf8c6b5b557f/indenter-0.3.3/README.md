## indenter

[![Build Status][actions-badge]][actions-url]
[![Latest Version][version-badge]][version-url]
[![Rust Documentation][docs-badge]][docs-url]

[actions-badge]: https://github.com/yaahc/indenter/workflows/Continuous%20integration/badge.svg
[actions-url]: https://github.com/yaahc/indenter/actions?query=workflow%3A%22Continuous+integration%22
[version-badge]: https://img.shields.io/crates/v/indenter.svg
[version-url]: https://crates.io/crates/indenter
[docs-badge]: https://img.shields.io/badge/docs-latest-blue.svg
[docs-url]: https://docs.rs/indenter

A few wrappers for the `fmt::Write` objects that efficiently appends and remove
common indentation after every newline

## Setup

Add this to your `Cargo.toml`:

```toml
[dependencies]
indenter = "0.2"
```

## Examples

## Indentation only

This type is intended primarily for writing error reporters that gracefully
format error messages that span multiple lines.

```rust
use std::error::Error;
use std::fmt::{self, Write};
use indenter::indented;

struct ErrorReporter<'a>(&'a dyn Error);

impl fmt::Debug for ErrorReporter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut source = Some(self.0);
        let mut i = 0;

        while let Some(error) = source {
            writeln!(f)?;
            write!(indented(f).ind(i), "{}", error)?;

            source = error.source();
            i += 1;
        }

        Ok(())
    }
}
```

## "Dedenting" (removing common leading indendation)

This type is intended primarily for formatting source code. For example, when
generating code.

This type requires the feature `std`.

```rust
use std::error::Error;
use core::fmt::{self, Write};
use indenter::CodeFormatter;

let mut output = String::new();
let mut f = CodeFormatter::new(&mut output, "    ");

write!(
    f,
    r#"
    Hello
        World
    "#,
);

assert_eq!(output, "Hello\n    World\n");

let mut output = String::new();
let mut f = CodeFormatter::new(&mut output, "    ");

// it can also indent...
f.indent(2);

write!(
    f,
    r#"
    Hello
        World
    "#,
);

assert_eq!(output, "        Hello\n            World\n");
```

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
