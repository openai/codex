![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/splash.svg

# tracing-error

Utilities for enriching error handling with [`tracing`] diagnostic
information.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]
![maintenance status][maint-badge]

[Documentation (release)][docs-url] | [Documentation (master)][docs-master-url] | [Chat][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tracing-error.svg
[crates-url]: https://crates.io/crates/tracing-error/0.2.1
[docs-badge]: https://docs.rs/tracing-error/badge.svg
[docs-url]: https://docs.rs/tracing-error/0.2.1/tracing_error
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tracing-rs.netlify.com/tracing_error
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw
[maint-badge]: https://img.shields.io/badge/maintenance-experimental-blue.svg

# Overview

[`tracing`] is a framework for instrumenting Rust programs to collect
scoped, structured, and async-aware diagnostics. This crate provides
integrations between [`tracing`] instrumentation and Rust error handling. It
enables enriching error types with diagnostic information from `tracing`
[span] contexts, formatting those contexts when errors are displayed, and
automatically generate `tracing` [events] when errors occur.

The crate provides the following:

* [`SpanTrace`], a captured trace of the current `tracing` [span] context

* [`ErrorLayer`], a [subscriber layer] which enables capturing `SpanTrace`s

**Note**: This crate is currently experimental.

*Compiler support: [requires `rustc` 1.63+][msrv]*

[msrv]: #supported-rust-versions

## Usage

`tracing-error` provides the [`SpanTrace`] type, which captures the current
`tracing` span context when it is constructed and allows it to be displayed
at a later time.

For example:

```rust
use std::{fmt, error::Error};
use tracing_error::SpanTrace;

#[derive(Debug)]
pub struct MyError {
    context: SpanTrace,
    // ...
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // ... format other parts of the error ...

        self.context.fmt(f)?;

        // ... format other error context information, cause chain, etc ...
        # Ok(())
    }
}

impl Error for MyError {}

impl MyError {
    pub fn new() -> Self {
        Self {
            context: SpanTrace::capture(),
            // ... other error information ...
        }
    }
}
```

This crate also provides [`TracedError`], for attaching a [`SpanTrace`] to an
existing error. The easiest way to wrap errors in `TracedError` is to either
use the [`InstrumentResult`] and [`InstrumentError`] traits or the `From`/`Into`
traits.

```rust
use tracing_error::prelude::*;

std::fs::read_to_string("myfile.txt").in_current_span()?;
```

Once an error has been wrapped with with a [`TracedError`], the [`SpanTrace`]
can be extracted one of three ways: either via [`TracedError`]'s
`Display`/`Debug` implementations, or via the [`ExtractSpanTrace`] trait.

For example, here is how one might print the errors but specialize the
printing when the error is a placeholder for a wrapping [`SpanTrace`]:

```rust
use std::error::Error;
use tracing_error::ExtractSpanTrace as _;

fn print_extracted_spantraces(error: &(dyn Error + 'static)) {
    let mut error = Some(error);
    let mut ind = 0;

    eprintln!("Error:");

    while let Some(err) = error {
        if let Some(spantrace) = err.span_trace() {
            eprintln!("found a spantrace:\n{}", spantrace);
        } else {
            eprintln!("{:>4}: {}", ind, err);
        }

        error = err.source();
        ind += 1;
    }
}

```

Whereas here, we can still display the content of the `SpanTraces` without
any special casing by simply printing all errors in our error chain.

```rust
use std::error::Error;

fn print_naive_spantraces(error: &(dyn Error + 'static)) {
    let mut error = Some(error);
    let mut ind = 0;

    eprintln!("Error:");

    while let Some(err) = error {
        eprintln!("{:>4}: {}", ind, err);
        error = err.source();
        ind += 1;
    }
}
```

Applications that wish to use `tracing-error`-enabled errors should
construct an [`ErrorLayer`] and add it to their [`Subscriber`] in order to
enable capturing [`SpanTrace`]s. For example:

```rust
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;

fn main() {
    let subscriber = tracing_subscriber::Registry::default()
        // any number of other subscriber layers may be added before or
        // after the `ErrorLayer`...
        .with(ErrorLayer::default());

    // set the subscriber as the default for the application
    tracing::subscriber::set_global_default(subscriber);
}
```

## Feature Flags

- `traced-error` - Enables the [`TracedError`] type and related traits
    - [`InstrumentResult`] and [`InstrumentError`] extension traits, which
    provide an [`in_current_span()`] method for bundling errors with a
    [`SpanTrace`].
    - [`ExtractSpanTrace`] extension trait, for extracting `SpanTrace`s from
    behind `dyn Error` trait objects.

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.63. The current Tracing version is not guaranteed to build on Rust
versions earlier than the minimum supported version.

Tracing follows the same compiler support policies as the rest of the Tokio
project. The current stable Rust compiler and the three most recent minor
versions before it will always be supported. For example, if the current stable
compiler version is 1.69, the minimum supported version will not be increased
past 1.66, three minor versions prior. Increasing the minimum supported compiler
version is not considered a semver breaking change as long as doing so complies
with this policy.

## Related Crates

In addition to this repository, here are also several third-party crates which
are not maintained by the `tokio` project. These include:

- [`color-spantrace`] provides a formatter for rendering SpanTrace in the style
  of `color-backtrace`
- [`color-eyre`] provides a customized version of `eyre::Report` for capturing
  span traces and backtraces with new errors and pretty printing them in error
  reports.

[`color-spantrace`]: https://github.com/yaahc/color-spantrace
[`color-eyre`]: https://github.com/yaahc/color-eyre

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tracing by you, shall be licensed as MIT, without any additional
terms or conditions.

[`SpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/struct.SpanTrace.html
[`ErrorLayer`]: https://docs.rs/tracing-error/*/tracing_error/struct.ErrorLayer.html
[`TracedError`]: https://docs.rs/tracing-error/*/tracing_error/struct.TracedError.html
[`InstrumentResult`]: https://docs.rs/tracing-error/*/tracing_error/trait.InstrumentResult.html
[`InstrumentError`]: https://docs.rs/tracing-error/*/tracing_error/trait.InstrumentError.html
[`ExtractSpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/trait.ExtractSpanTrace.html
[`in_current_span()`]: https://docs.rs/tracing-error/*/tracing_error/trait.InstrumentResult.html#tymethod.in_current_span
[span]: https://docs.rs/tracing/latest/tracing/span/index.html
[events]: https://docs.rs/tracing/latest/tracing/struct.Event.html
[`Subscriber`]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
[subscriber layer]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html
[`tracing`]: https://docs.rs/tracing
[`std::error::Error`]: https://doc.rust-lang.org/stable/std/error/trait.Error.html
[`SpanTrace`]: https://docs.rs/tracing-error/0.2.1/tracing_error/struct.SpanTrace.html
[`ErrorLayer`]: https://docs.rs/tracing-error/0.2.1/tracing_error/struct.ErrorLayer.html
