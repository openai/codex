derive(Display) /// `From<docs>`
===============

[![Latest Version](https://img.shields.io/crates/v/displaydoc.svg)](https://crates.io/crates/displaydoc)
[![Rust Documentation](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/displaydoc)

This library provides a convenient derive macro for the standard library's
[`core::fmt::Display`] trait.

[`core::fmt::Display`]: https://doc.rust-lang.org/std/fmt/trait.Display.html

```toml
[dependencies]
displaydoc = "0.2"
```

*Compiler support: requires rustc 1.56+*

<br>

### Example

*Demonstration alongside the [`Error`][std::error::Error] derive macro from [`thiserror`](https://docs.rs/thiserror/1.0.25/thiserror/index.html),
to propagate source locations from [`io::Error`][std::io::Error] with the `#[source]` attribute:*
```rust
use std::io;
use displaydoc::Display;
use thiserror::Error;

#[derive(Display, Error, Debug)]
pub enum DataStoreError {
    /// data store disconnected
    Disconnect(#[source] io::Error),
    /// the data for key `{0}` is not available
    Redaction(String),
    /// invalid header (expected {expected:?}, found {found:?})
    InvalidHeader {
        expected: String,
        found: String,
    },
    /// unknown data store error
    Unknown,
}

let error = DataStoreError::Redaction("CLASSIFIED CONTENT".to_string());
assert!("the data for key `CLASSIFIED CONTENT` is not available" == &format!("{}", error));
```
*Note that although [`io::Error`][std::io::Error] implements `Display`, we do not add it to the
generated message for `DataStoreError::Disconnect`, since it is already made available via
`#[source]`. See further context on avoiding duplication in error reports at the rust blog
[here](https://github.com/yaahc/blog.rust-lang.org/blob/master/posts/inside-rust/2021-05-15-What-the-error-handling-project-group-is-working-towards.md#duplicate-information-issue).*

<br>

### Details

- A `fmt::Display` impl is generated for your enum if you provide
  a docstring comment on each variant as shown above in the example. The
  `Display` derive macro supports a shorthand for interpolating fields from
  the error:
    - `/// {var}` ⟶ `write!("{}", self.var)`
    - `/// {0}` ⟶ `write!("{}", self.0)`
    - `/// {var:?}` ⟶ `write!("{:?}", self.var)`
    - `/// {0:?}` ⟶ `write!("{:?}", self.0)`
- This also works with structs and [generic types][crate::Display#generic-type-parameters]:
```rust
/// oh no, an error: {0}
#[derive(Display)]
pub struct Error<E>(pub E);

let error: Error<&str> = Error("muahaha i am an error");
assert!("oh no, an error: muahaha i am an error" == &format!("{}", error));
```

- Two optional attributes can be added to your types next to the derive:

    - `#[ignore_extra_doc_attributes]` makes the macro ignore any doc
      comment attributes (or `///` lines) after the first. Multi-line
      comments using `///` are otherwise treated as an error, so use this
      attribute or consider switching to block doc comments (`/** */`).

    - `#[prefix_enum_doc_attributes]` combines the doc comment message on
      your enum itself with the messages for each variant, in the format
      “enum: variant”. When added to an enum, the doc comment on the enum
      becomes mandatory. When added to any other type, it has no effect.

- In case you want to have an independent doc comment, the
  `#[displaydoc("...")` atrribute may be used on the variant or struct to
  override it.

<br>

### FAQ

1. **Is this crate `no_std` compatible?**
    * Yes! This crate implements the [`core::fmt::Display`] trait, not the [`std::fmt::Display`] trait, so it should work in `std` and `no_std` environments. Just add `default-features = false`.

2. **Does this crate work with `Path` and `PathBuf` via the `Display` trait?**
    * Yuuup. This crate uses @dtolnay's [autoref specialization technique](https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md) to add a special trait for types to get the display impl. It then specializes for `Path` and `PathBuf`, and when either of these types are found, it calls `self.display()` to get a `std::path::Display<'_>` type which can be used with the `Display` format specifier!


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
