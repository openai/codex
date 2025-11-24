[![pipeline](https://github.com/d-e-s-o/test-log/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/d-e-s-o/test-log/actions/workflows/test.yml)
[![crates.io](https://img.shields.io/crates/v/test-log.svg)](https://crates.io/crates/test-log)
[![Docs](https://docs.rs/test-log/badge.svg)][docs-rs]
[![rustc](https://img.shields.io/badge/rustc-1.71+-blue.svg)](https://blog.rust-lang.org/2023/07/13/Rust-1.71.0.html)

test-log
========

- [Documentation][docs-rs]
- [Changelog](CHANGELOG.md)

**test-log** is a crate that takes care of automatically initializing
logging and/or tracing for Rust tests.

When running Rust tests it can often be helpful to have easy access to
the verbose log messages emitted by the code under test. Commonly, these
log messages may be coming from the [`log`][log] crate or being emitted
through the [`tracing`][tracing] infrastructure.

The problem with either -- in the context of testing -- is that some
form of initialization is required in order to make these crate's
messages appear on a standard output stream.

The commonly used [`env_logger`](https://crates.io/crates/env_logger)
(which provides an easy way to configure `log` based logging), for
example, needs to be initialized like this:
```rust
let _ = env_logger::builder().is_test(true).try_init();
```
in **each and every** test.

Similarly, `tracing` based solutions require a subscriber to be
registered that writes events/spans to the terminal.

This crate takes care of this per-test initialization in an intuitive
way.


Usage
-----

The crate provides a custom `#[test]` attribute that, when used for
running a particular test, takes care of initializing `log` and/or
`tracing` beforehand.

#### Example

As such, usage is as simple as importing and using said attribute:
```rust
use test_log::test;

#[test]
fn it_works() {
  info!("Checking whether it still works...");
  assert_eq!(2 + 2, 4);
  info!("Looks good!");
}
```

It is of course also possible to initialize logging for a chosen set of
tests, by only annotating these with the custom attribute:
```rust
#[test_log::test]
fn it_still_works() {
  // ...
}
```

The crate also supports stacking with other `#[test]` attributes.
For example, you can stack [`#[tokio::test]`][tokio-test] on top of this
crate's `#[test]` attribute and test `async` functionality this way:

```rust
#[tokio::test]
#[test_log::test]
async fn it_still_works() {
  // ...
}
```

Note that stacking `#[test]` attributes this way requires some minimal
level of cooperation from the other crate to work properly (see
[#46](https://github.com/d-e-s-o/test-log/pull/46) for details), but as
a fallback a wrapping style can be used as well:
```rust
use test_log::test;

#[test(tokio::test)]
async fn it_also_works() {
  // ...
}
```

#### Cargo Feature Flags

```toml
# Cargo.toml
[dependencies]
# PICK ONE OF THE FOLLOWING:

# Support `log` crate only (default).
test-log = {version = "0.2"}
# Support `log` and `tracing` crates.
test-log = {version = "0.2", features = ["trace"]}
# Support only `tracing` crate.
test-log = {version = "0.2", default-features = false, features = ["trace"]}
```

The crate comes with two features pertaining "backend" initialization:
- `log`, enabled by default, controls initialization for the `log`
  crate.
- `trace`, disabled by default, controls initialization for the
  `tracing` crate.

Depending on what backend the crate-under-test (and its dependencies)
use, the respective feature(s) should be enabled to make messages that
are emitted by the test manifest on the terminal.

On top of that, the `color` feature (enabled by default) controls
whether to color output by default.

#### Logging Configuration

As usual when running `cargo test`, the output is captured by the
framework by default and only shown on test failure. The `--nocapture`
argument can be supplied in order to overwrite this setting. E.g.,
```bash
$ cargo test -- --nocapture
```

Furthermore, the `RUST_LOG` environment variable is honored and can be
used to influence the log level to work with (among other things). By
default, log messages of criticality `INFO` (and higher) will be
emitted. Please refer to the [`env_logger` docs][env-docs-rs] and
[`tracing-subscriber`][tracing-env-docs-rs] documentation for supported
variable syntax and more information.

If the `trace` feature is enabled, the `RUST_LOG_SPAN_EVENTS`
environment variable can be used to configure the tracing subscriber to
log synthesized events at points in the span lifecycle. Set the variable
to a comma-separated list of events you want to see. For example,
`RUST_LOG_SPAN_EVENTS=full` or `RUST_LOG_SPAN_EVENTS=new,close`.

Valid events are `new`, `enter`, `exit`, `close`, `active`, and `full`.
See the [`tracing_subscriber` docs][tracing-events-docs-rs] for details
on what the events mean.

#### MSRV Policy
This crate adheres to Cargo's [semantic versioning rules][cargo-semver].
At a minimum, it builds with the most recent Rust stable release minus
five minor versions ("N - 5"). E.g., assuming the most recent Rust
stable is `1.68`, the crate is guaranteed to build with `1.63` and
higher.

[cargo-semver]: https://doc.rust-lang.org/cargo/reference/resolver.html#semver-compatibility
[docs-rs]: https://docs.rs/test-log
[env-docs-rs]: https://docs.rs/env_logger/0.11.2/env_logger
[log]: https://crates.io/crates/log
[tokio-test]: https://docs.rs/tokio/1.45.1/tokio/attr.test.html
[tracing]: https://crates.io/crates/tracing
[tracing-env-docs-rs]: https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/filter/struct.EnvFilter.html#directives
[tracing-events-docs-rs]: https://docs.rs/tracing-subscriber/0.3.18/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_span_events
