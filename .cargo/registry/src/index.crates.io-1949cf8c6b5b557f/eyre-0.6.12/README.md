eyre
====

[![Build Status][actions-badge]][actions-url]
[![Latest Version](https://img.shields.io/crates/v/eyre.svg)](https://crates.io/crates/eyre)
[![Rust Documentation](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/eyre)
[![Discord chat][discord-badge]][discord-url]

[actions-badge]: https://github.com/eyre-rs/eyre/workflows/Continuous%20integration/badge.svg
[actions-url]: https://github.com/eyre-rs/eyre/actions?query=workflow%3A%22Continuous+integration%22
[discord-badge]: https://img.shields.io/discord/960645145018110012?label=eyre%20community%20discord
[discord-url]: https://discord.gg/z94RqmUTKB

This library provides [`eyre::Report`][Report], a trait object based
error handling type for easy idiomatic error handling and reporting in Rust
applications.

This crate is a fork of [`anyhow`]  with support for customized
error reports. For more details on customization checkout the docs on
[`eyre::EyreHandler`].

## Custom Report Handlers

The heart of this crate is its ability to swap out the Handler type to change
what information is carried alongside errors and how the end report is
formatted. This crate is meant to be used alongside companion crates that
customize its behavior. Below is a list of known crates that export report
handlers for eyre and short summaries of what features they provide.

- [`stable-eyre`]: Switches the backtrace type from `std`'s to `backtrace-rs`'s
  so that it can be captured on stable. The report format is identical to
  `DefaultHandler`'s report format.
- [`color-eyre`]: Captures a `backtrace::Backtrace` and a
  `tracing_error::SpanTrace`. Provides a `Help` trait for attaching warnings
  and suggestions to error reports. The end report is then pretty printed with
  the help of [`color-backtrace`], [`color-spantrace`], and `ansi_term`. Check
  out the README on [`color-eyre`] for details on the report format.
- [`simple-eyre`]: A minimal `EyreHandler` that captures no additional
  information, for when you do not wish to capture `Backtrace`s with errors.
- [`jane-eyre`]: A report handler crate that exists purely for the pun of it.
  Currently just re-exports `color-eyre`.

## Usage Recommendations and Stability Considerations

**We recommend users do not re-export types from this library as part their own
public API for libraries with external users.** The main reason for this is
that it will make your library API break if we ever bump the major version
number on eyre and your users upgrade the eyre version they use in their
application code before you upgrade your own eyre dep version[^1].

However, even beyond this API stability hazard, there are other good reasons to
avoid using `eyre::Report` as your public error type.

- You export an undocumented error interface that is otherwise still accessible
  via downcast, making it hard for users to react to specific errors while not
  preventing them from depending on details you didn't mean to make part of
  your public API.
  - This in turn makes the error types of all libraries you use a part of your
    public API as well, and makes changing any of those libraries into an
    undetectable runtime breakage.
- If many of your errors are constructed from strings you encourage your users
  to use string comparision for reacting to specific errors which is brittle
  and turns updating error messages into a potentially undetectable runtime
  breakage.

## Details

- Use `Result<T, eyre::Report>`, or equivalently `eyre::Result<T>`, as the
  return type of any fallible function.

  Within the function, use `?` to easily propagate any error that implements the
  `std::error::Error` trait.

  ```rust
  use eyre::Result;

  fn get_cluster_info() -> Result<ClusterMap> {
      let config = std::fs::read_to_string("cluster.json")?;
      let map: ClusterMap = serde_json::from_str(&config)?;
      Ok(map)
  }
  ```

- Wrap a lower level error with a new error created from a message to help the
  person troubleshooting understand the chain of failures that occurred. A
  low-level error like "No such file or directory" can be annoying to debug
  without more information about what higher level step the application was in
  the middle of.

  ```rust
  use eyre::{WrapErr, Result};

  fn main() -> Result<()> {
      ...
      it.detach().wrap_err("Failed to detach the important thing")?;

      let content = std::fs::read(path)
          .wrap_err_with(|| format!("Failed to read instrs from {}", path))?;
      ...
  }
  ```

  ```console
  Error: Failed to read instrs from ./path/to/instrs.json

  Caused by:
      No such file or directory (os error 2)
  ```

- Downcasting is supported and can be by value, by shared reference, or by
  mutable reference as needed.

  ```rust
  // If the error was caused by redaction, then return a
  // tombstone instead of the content.
  match root_cause.downcast_ref::<DataStoreError>() {
      Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
      None => Err(error),
  }
  ```

- If using the nightly channel, a backtrace is captured and printed with the
  error if the underlying error type does not already provide its own. In order
  to see backtraces, they must be enabled through the environment variables
  described in [`std::backtrace`]:

  - If you want panics and errors to both have backtraces, set
    `RUST_BACKTRACE=1`;
  - If you want only errors to have backtraces, set `RUST_LIB_BACKTRACE=1`;
  - If you want only panics to have backtraces, set `RUST_BACKTRACE=1` and
    `RUST_LIB_BACKTRACE=0`.

  The tracking issue for this feature is [rust-lang/rust#53487].

  [`std::backtrace`]: https://doc.rust-lang.org/std/backtrace/index.html#environment-variables
  [rust-lang/rust#53487]: https://github.com/rust-lang/rust/issues/53487

- Eyre works with any error type that has an impl of `std::error::Error`,
  including ones defined in your crate. We do not bundle a `derive(Error)` macro
  but you can write the impls yourself or use a standalone macro like
  [thiserror].

  ```rust
  use thiserror::Error;

  #[derive(Error, Debug)]
  pub enum FormatError {
      #[error("Invalid header (expected {expected:?}, got {found:?})")]
      InvalidHeader {
          expected: String,
          found: String,
      },
      #[error("Missing attribute: {0}")]
      MissingAttribute(String),
  }
  ```

- One-off error messages can be constructed using the `eyre!` macro, which
  supports string interpolation and produces an `eyre::Report`.

  ```rust
  return Err(eyre!("Missing attribute: {}", missing));
  ```

- On newer versions of the compiler (e.g. 1.58 and later) this macro also
  supports format args captures.

  ```rust
  return Err(eyre!("Missing attribute: {missing}"));
  ```

## No-std support

No-std support was removed in 2020 in [commit 608a16a] due to unaddressed upstream breakages.
[commit 608a16a]:
https://github.com/eyre-rs/eyre/pull/29/commits/608a16aa2c2c27eca6c88001cc94c6973c18f1d5

## Comparison to failure

The `eyre::Report` type works something like `failure::Error`, but unlike
failure ours is built around the standard library's `std::error::Error` trait
rather than a separate trait `failure::Fail`. The standard library has adopted
the necessary improvements for this to be possible as part of [RFC 2504].

[RFC 2504]: https://github.com/rust-lang/rfcs/blob/master/text/2504-fix-error.md

## Comparison to thiserror

Use `eyre` if you don't think you'll do anything with an error other than
report it. This is common in application code. Use `thiserror` if you think
you need an error type that can be handled via match or reported. This is
common in library crates where you don't know how your users will handle
your errors.

[thiserror]: https://github.com/dtolnay/thiserror

## Compatibility with `anyhow`

This crate does its best to be usable as a drop in replacement of `anyhow` and
vice-versa by `re-exporting` all of the renamed APIs with the names used in
`anyhow`, though there are some differences still.

#### `Context` and `Option`

As part of renaming `Context` to `WrapErr` we also intentionally do not
implement `WrapErr` for `Option`. This decision was made because `wrap_err`
implies that you're creating a new error that saves the old error as its
`source`. With `Option` there is no source error to wrap, so `wrap_err` ends up
being somewhat meaningless.

Instead `eyre` offers [`OptionExt::ok_or_eyre`] to yield _static_ errors from `None`,
and intends for users to use the combinator functions provided by
`std`, converting `Option`s to `Result`s, for _dynamic_ errors.
So where you would write this with
anyhow:

[`OptionExt::ok_or_eyre`]: https://docs.rs/eyre/latest/eyre/trait.OptionExt.html#tymethod.ok_or_eyre

```rust
use anyhow::Context;

let opt: Option<()> = None;
let result_static = opt.context("static error message");
let result_dynamic = opt.with_context(|| format!("{} error message", "dynamic"));
```

With `eyre` we want users to write:

```rust
use eyre::{eyre, OptionExt, Result};

let opt: Option<()> = None;
let result_static: Result<()> = opt.ok_or_eyre("static error message");
let result_dynamic: Result<()> = opt.ok_or_else(|| eyre!("{} error message", "dynamic"));
```

**NOTE**: However, to help with porting we do provide a `ContextCompat` trait which
implements `context` for options which you can import to make existing
`.context` calls compile.

[Report]: https://docs.rs/eyre/*/eyre/struct.Report.html
[`eyre::EyreHandler`]: https://docs.rs/eyre/*/eyre/trait.EyreHandler.html
[`eyre::WrapErr`]: https://docs.rs/eyre/*/eyre/trait.WrapErr.html
[`anyhow::Context`]: https://docs.rs/anyhow/*/anyhow/trait.Context.html
[`anyhow`]: https://github.com/dtolnay/anyhow
[`tracing_error::SpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/struct.SpanTrace.html
[`stable-eyre`]: https://github.com/eyre-rs/stable-eyre
[`color-eyre`]: https://github.com/eyre-rs/color-eyre
[`jane-eyre`]: https://github.com/yaahc/jane-eyre
[`simple-eyre`]: https://github.com/eyre-rs/simple-eyre
[`color-spantrace`]: https://github.com/eyre-rs/color-spantrace
[`color-backtrace`]: https://github.com/athre0z/color-backtrace

[^1]: example and explanation of breakage https://github.com/eyre-rs/eyre/issues/30#issuecomment-647650361

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
