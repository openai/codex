# color-spantrace

[![Build Status][actions-badge]][actions-url]
[![Latest Version][version-badge]][version-url]
[![Rust Documentation][docs-badge]][docs-url]

[actions-badge]: https://github.com/eyre-rs/eyre/workflows/Continuous%20integration/badge.svg
[actions-url]: https://github.com/eyre-rs/eyre/actions?query=workflow%3A%22Continuous+integration%22
[version-badge]: https://img.shields.io/crates/v/color-spantrace.svg
[version-url]: https://crates.io/crates/color-spantrace
[docs-badge]: https://img.shields.io/badge/docs-latest-blue.svg
[docs-url]: https://docs.rs/color-spantrace

A rust library for colorizing [`tracing_error::SpanTrace`] objects in the style
of [`color-backtrace`].

## Setup

Add the following to your `Cargo.toml`:

```toml
[dependencies]
color-spantrace = "0.2"
tracing = "0.1"
tracing-error = "0.2"
tracing-subscriber = "0.3"
```

Setup a tracing subscriber with an `ErrorLayer`:

```rust
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, registry::Registry};

Registry::default().with(ErrorLayer::default()).init();
```

Create spans and enter them:

```rust
use tracing::instrument;
use tracing_error::SpanTrace;

#[instrument]
fn foo() -> SpanTrace {
    SpanTrace::capture()
}
```

And finally colorize the `SpanTrace`:

```rust
use tracing_error::SpanTrace;

let span_trace = SpanTrace::capture();
println!("{}", color_spantrace::colorize(&span_trace));
```

## Example

This example is taken from `examples/color-spantrace-usage.rs`:

```rust
use tracing::instrument;
use tracing_error::{ErrorLayer, SpanTrace};
use tracing_subscriber::{prelude::*, registry::Registry};

#[instrument]
fn main() {
    Registry::default().with(ErrorLayer::default()).init();

    let span_trace = one(42);
    println!("{}", color_spantrace::colorize(&span_trace));
}

#[instrument]
fn one(i: u32) -> SpanTrace {
    two()
}

#[instrument]
fn two() -> SpanTrace {
    SpanTrace::capture()
}
```

This creates the following output

### Minimal Format

![minimal format](https://raw.githubusercontent.com/eyre-rs/eyre/refs/heads/master/color-spantrace/pictures/minimal.png)

### Full Format

![Full format](https://raw.githubusercontent.com/eyre-rs/eyre/refs/heads/master/color-spantrace/pictures/full.png)

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

[`tracing_error::SpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/struct.SpanTrace.html
[`color-backtrace`]: https://github.com/athre0z/color-backtrace
