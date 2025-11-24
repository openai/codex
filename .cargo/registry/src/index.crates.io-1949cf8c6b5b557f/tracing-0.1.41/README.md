![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/splash.svg

# tracing

Application-level tracing for Rust.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Documentation][docs-url] | [Chat][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tracing.svg
[crates-url]: https://crates.io/crates/tracing
[docs-badge]: https://docs.rs/tracing/badge.svg
[docs-url]: https://docs.rs/tracing
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tracing-rs.netlify.com/tracing
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw

## Overview

`tracing` is a framework for instrumenting Rust programs to collect
structured, event-based diagnostic information.

In asynchronous systems like Tokio, interpreting traditional log messages can
often be quite challenging. Since individual tasks are multiplexed on the same
thread, associated events and log lines are intermixed making it difficult to
trace the logic flow. `tracing` expands upon logging-style diagnostics by
allowing libraries and applications to record structured events with additional
information about *temporality* and *causality* — unlike a log message, a span
in `tracing` has a beginning and end time, may be entered and exited by the
flow of execution, and may exist within a nested tree of similar spans. In
addition, `tracing` spans are *structured*, with the ability to record typed
data as well as textual messages.

The `tracing` crate provides the APIs necessary for instrumenting libraries
and applications to emit trace data.

*Compiler support: [requires `rustc` 1.63+][msrv]*

[msrv]: #supported-rust-versions

## Usage

(The examples below are borrowed from the `log` crate's yak-shaving
[example](https://docs.rs/log/0.4.10/log/index.html#examples), modified to
idiomatic `tracing`.)

### In Applications

In order to record trace events, executables have to use a `Subscriber`
implementation compatible with `tracing`. A `Subscriber` implements a way of
collecting trace data, such as by logging it to standard output. [`tracing_subscriber`](https://docs.rs/tracing-subscriber/)'s
[`fmt` module](https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/fmt/index.html) provides reasonable defaults.
Additionally, `tracing-subscriber` is able to consume messages emitted by `log`-instrumented libraries and modules.

The simplest way to use a subscriber is to call the `set_global_default` function.

```rust
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let number_of_yaks = 3;
    // this creates a new event, outside of any spans.
    info!(number_of_yaks, "preparing to shave yaks");

    let number_shaved = yak_shave::shave_all(number_of_yaks);
    info!(
        all_yaks_shaved = number_shaved == number_of_yaks,
        "yak shaving completed."
    );
}
```

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3.0"
```

This subscriber will be used as the default in all threads for the remainder of the duration
of the program, similar to how loggers work in the `log` crate.

In addition, you can locally override the default subscriber. For example:

```rust
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        // builds the subscriber.
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        info!("This will be logged to stdout");
    });
    info!("This will _not_ be logged to stdout");
}
```

This approach allows trace data to be collected by multiple subscribers
within different contexts in the program. Note that the override only applies to the
currently executing thread; other threads will not see the change from with_default.

Any trace events generated outside the context of a subscriber will not be collected.

Once a subscriber has been set, instrumentation points may be added to the
executable using the `tracing` crate's macros.

### In Libraries

Libraries should only rely on the `tracing` crate and use the provided macros
and types to collect whatever information might be useful to downstream consumers.

```rust
use std::{error::Error, io};
use tracing::{debug, error, info, span, warn, Level};

// the `#[tracing::instrument]` attribute creates and enters a span
// every time the instrumented function is called. The span is named after the
// the function or method. Parameters passed to the function are recorded as fields.
#[tracing::instrument]
pub fn shave(yak: usize) -> Result<(), Box<dyn Error + 'static>> {
    // this creates an event at the DEBUG level with two fields:
    // - `excitement`, with the key "excitement" and the value "yay!"
    // - `message`, with the key "message" and the value "hello! I'm gonna shave a yak."
    //
    // unlike other fields, `message`'s shorthand initialization is just the string itself.
    debug!(excitement = "yay!", "hello! I'm gonna shave a yak.");
    if yak == 3 {
        warn!("could not locate yak!");
        // note that this is intended to demonstrate `tracing`'s features, not idiomatic
        // error handling! in a library or application, you should consider returning
        // a dedicated `YakError`. libraries like snafu or thiserror make this easy.
        return Err(io::Error::new(io::ErrorKind::Other, "shaving yak failed!").into());
    } else {
        debug!("yak shaved successfully");
    }
    Ok(())
}

pub fn shave_all(yaks: usize) -> usize {
    // Constructs a new span named "shaving_yaks" at the TRACE level,
    // and a field whose key is "yaks". This is equivalent to writing:
    //
    // let span = span!(Level::TRACE, "shaving_yaks", yaks = yaks);
    //
    // local variables (`yaks`) can be used as field values
    // without an assignment, similar to struct initializers.
    let _span_ = span!(Level::TRACE, "shaving_yaks", yaks).entered();

    info!("shaving yaks");

    let mut yaks_shaved = 0;
    for yak in 1..=yaks {
        let res = shave(yak);
        debug!(yak, shaved = res.is_ok());

        if let Err(ref error) = res {
            // Like spans, events can also use the field initialization shorthand.
            // In this instance, `yak` is the field being initialized.
            error!(yak, error = error.as_ref(), "failed to shave yak!");
        } else {
            yaks_shaved += 1;
        }
        debug!(yaks_shaved);
    }

    yaks_shaved
}
```

```toml
[dependencies]
tracing = "0.1"
```

Note: Libraries should *NOT* call `set_global_default()`, as this will cause
conflicts when executables try to set the default later.

### In Asynchronous Code

If you are instrumenting code that make use of
[`std::future::Future`](https://doc.rust-lang.org/stable/std/future/trait.Future.html)
or async/await, avoid using the `Span::enter` method. The following example
_will not_ work:

```rust
async {
    let _s = span.enter();
    // ...
}
```
```rust
async {
    let _s = tracing::span!(...).entered();
    // ...
}
```

The span guard `_s` will not exit until the future generated by the `async` block is complete.
Since futures and spans can be entered and exited _multiple_ times without them completing,
the span remains entered for as long as the future exists, rather than being entered only when
it is polled, leading to very confusing and incorrect output.
For more details, see [the documentation on closing spans](https://tracing.rs/tracing/span/index.html#closing-spans).

There are two ways to instrument asynchronous code. The first is through the
[`Future::instrument`](https://docs.rs/tracing/latest/tracing/trait.Instrument.html#method.instrument) combinator:

```rust
use tracing::Instrument;

let my_future = async {
    // ...
};

my_future
    .instrument(tracing::info_span!("my_future"))
    .await
```

`Future::instrument` attaches a span to the future, ensuring that the span's lifetime
is as long as the future's.

The second, and preferred, option is through the
[`#[instrument]`](https://docs.rs/tracing/0.1.41/tracing/attr.instrument.html)
attribute:

```rust
use tracing::{info, instrument};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use std::io;

#[instrument]
async fn write(stream: &mut TcpStream) -> io::Result<usize> {
    let result = stream.write(b"hello world\n").await;
    info!("wrote to stream; success={:?}", result.is_ok());
    result
}
```

Under the hood, the `#[instrument]` macro performs the same explicit span
attachment that `Future::instrument` does.

### Concepts

This crate provides macros for creating `Span`s and `Event`s, which represent
periods of time and momentary events within the execution of a program,
respectively.

As a rule of thumb, _spans_ should be used to represent discrete units of work
(e.g., a given request's lifetime in a server) or periods of time spent in a
given context (e.g., time spent interacting with an instance of an external
system, such as a database). In contrast, _events_ should be used to represent
points in time within a span — a request returned with a given status code,
_n_ new items were taken from a queue, and so on.

`Span`s are constructed using the `span!` macro, and then _entered_
to indicate that some code takes place within the context of that `Span`:

```rust
use tracing::{span, Level};

// Construct a new span named "my span".
let mut span = span!(Level::INFO, "my span");
span.in_scope(|| {
    // Any trace events in this closure or code called by it will occur within
    // the span.
});
// Dropping the span will close it, indicating that it has ended.
```

The [`#[instrument]`](https://docs.rs/tracing/0.1.41/tracing/attr.instrument.html) attribute macro
can reduce some of this boilerplate:

```rust
use tracing::{instrument};

#[instrument]
pub fn my_function(my_arg: usize) {
    // This event will be recorded inside a span named `my_function` with the
    // field `my_arg`.
    tracing::info!("inside my_function!");
    // ...
}
```

The `Event` type represent an event that occurs instantaneously, and is
essentially a `Span` that cannot be entered. They are created using the `event!`
macro:

```rust
use tracing::{event, Level};

event!(Level::INFO, "something has happened!");
```

Users of the [`log`] crate should note that `tracing` exposes a set of macros for
creating `Event`s (`trace!`, `debug!`, `info!`, `warn!`, and `error!`) which may
be invoked with the same syntax as the similarly-named macros from the `log`
crate. Often, the process of converting a project to use `tracing` can begin
with a simple drop-in replacement.

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.42. The current Tracing version is not guaranteed to build on Rust
versions earlier than the minimum supported version.

Tracing follows the same compiler support policies as the rest of the Tokio
project. The current stable Rust compiler and the three most recent minor
versions before it will always be supported. For example, if the current stable
compiler version is 1.45, the minimum supported version will not be increased
past 1.42, three minor versions prior. Increasing the minimum supported compiler
version is not considered a semver breaking change as long as doing so complies
with this policy.

## Ecosystem

### Related Crates

In addition to `tracing` and `tracing-core`, the [`tokio-rs/tracing`] repository
contains several additional crates designed to be used with the `tracing` ecosystem.
This includes a collection of `Subscriber` implementations, as well as utility
and adapter crates to assist in writing `Subscriber`s and instrumenting
applications.

In particular, the following crates are likely to be of interest:

- [`tracing-futures`] provides a compatibility layer with the `futures`
  crate, allowing spans to be attached to `Future`s, `Stream`s, and `Executor`s.
- [`tracing-subscriber`] provides `Subscriber` implementations and
  utilities for working with `Subscriber`s. This includes a [`FmtSubscriber`]
  `FmtSubscriber` for logging formatted trace data to stdout, with similar
  filtering and formatting to the [`env_logger`] crate.
- [`tracing-log`] provides a compatibility layer with the [`log`] crate,
  allowing log messages to be recorded as `tracing` `Event`s within the
  trace tree. This is useful when a project using `tracing` have
  dependencies which use `log`. Note that if you're using
  `tracing-subscriber`'s `FmtSubscriber`, you don't need to depend on
  `tracing-log` directly.

Additionally, there are also several third-party crates which are not
maintained by the `tokio` project. These include:

- [`tracing-timing`] implements inter-event timing metrics on top of `tracing`.
  It provides a subscriber that records the time elapsed between pairs of
  `tracing` events and generates histograms.
- [`tracing-opentelemetry`] provides a subscriber for emitting traces to
  [OpenTelemetry]-compatible distributed tracing systems.
- [`tracing-honeycomb`] Provides a layer that reports traces spanning multiple machines to [honeycomb.io]. Backed by [`tracing-distributed`].
- [`tracing-distributed`] Provides a generic implementation of a layer that reports traces spanning multiple machines to some backend.
- [`tracing-actix`] provides `tracing` integration for the `actix` actor
  framework.
- [`axum-insights`] provides `tracing` integration and Application insights export for the `axum` web framework.
- [`tracing-gelf`] implements a subscriber for exporting traces in Greylog
  GELF format.
- [`tracing-coz`] provides integration with the [coz] causal profiler
  (Linux-only).
- [`test-log`] takes care of initializing `tracing` for tests, based on
  environment variables with an `env_logger` compatible syntax.
- [`tracing-unwrap`] provides convenience methods to report failed unwraps on `Result` or `Option` types to a `Subscriber`.
- [`diesel-tracing`] provides integration with [`diesel`] database connections.
- [`tracing-tracy`] provides a way to collect [Tracy] profiles in instrumented
  applications.
- [`tracing-elastic-apm`] provides a layer for reporting traces to [Elastic APM].
- [`tracing-etw`] provides a layer for emitting Windows [ETW] events.
- [`tracing-fluent-assertions`] provides a fluent assertions-style testing
  framework for validating the behavior of `tracing` spans.
- [`sentry-tracing`] provides a layer for reporting events and traces to [Sentry].
- [`tracing-loki`] provides a layer for shipping logs to [Grafana Loki].
- [`tracing-logfmt`] provides a layer that formats events and spans into the logfmt format.
- [`json-subscriber`] provides a layer for emitting JSON logs. The output can be customized much more than with [`FmtSubscriber`]'s JSON output.

If you're the maintainer of a `tracing` ecosystem crate not listed above,
please let us know! We'd love to add your project to the list!

[`tracing-timing`]: https://crates.io/crates/tracing-timing
[`tracing-opentelemetry`]: https://crates.io/crates/tracing-opentelemetry
[OpenTelemetry]: https://opentelemetry.io/
[`tracing-honeycomb`]: https://crates.io/crates/tracing-honeycomb
[`tracing-distributed`]: https://crates.io/crates/tracing-distributed
[honeycomb.io]: https://www.honeycomb.io/
[`tracing-actix`]: https://crates.io/crates/tracing-actix
[`axum-insights`]: https://crates.io/crates/axum-insights
[`tracing-gelf`]: https://crates.io/crates/tracing-gelf
[`tracing-coz`]: https://crates.io/crates/tracing-coz
[coz]: https://github.com/plasma-umass/coz
[`test-log`]: https://crates.io/crates/test-log
[`tracing-unwrap`]: https://docs.rs/tracing-unwrap
[`diesel`]: https://crates.io/crates/diesel
[`diesel-tracing`]: https://crates.io/crates/diesel-tracing
[`tracing-tracy`]: https://crates.io/crates/tracing-tracy
[Tracy]: https://github.com/wolfpld/tracy
[`tracing-elastic-apm`]: https://crates.io/crates/tracing-elastic-apm
[Elastic APM]: https://www.elastic.co/apm
[`tracing-etw`]: https://github.com/microsoft/tracing-etw
[ETW]: https://docs.microsoft.com/en-us/windows/win32/etw/about-event-tracing
[`tracing-fluent-assertions`]: https://crates.io/crates/tracing-fluent-assertions
[`sentry-tracing`]: https://crates.io/crates/sentry-tracing
[Sentry]: https://sentry.io/welcome/
[`tracing-loki`]: https://crates.io/crates/tracing-loki
[Grafana Loki]: https://grafana.com/oss/loki/
[`tracing-logfmt`]: https://crates.io/crates/tracing-logfmt
[`json-subscriber`]: https://crates.io/crates/json-subscriber

**Note:** that some of the ecosystem crates are currently unreleased and
undergoing active development. They may be less stable than `tracing` and
`tracing-core`.

[`log`]: https://docs.rs/log/0.4.6/log/
[`tokio-rs/tracing`]: https://github.com/tokio-rs/tracing
[`tracing-futures`]: https://github.com/tokio-rs/tracing/tree/master/tracing-futures
[`tracing-subscriber`]: https://github.com/tokio-rs/tracing/tree/master/tracing-subscriber
[`tracing-log`]: https://github.com/tokio-rs/tracing/tree/master/tracing-log
[`env_logger`]: https://crates.io/crates/env_logger
[`FmtSubscriber`]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.Subscriber.html
[`examples`]: https://github.com/tokio-rs/tracing/tree/master/examples

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

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
