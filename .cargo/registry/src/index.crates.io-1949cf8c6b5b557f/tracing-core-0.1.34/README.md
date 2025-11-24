![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/main/assets/splash.svg

# tracing-core

Core primitives for application-level tracing.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (v0.2.x)][docs-v0.2.x-badge]][docs-v0.2.x-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Documentation][docs-url] | [Chat][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tracing-core.svg
[crates-url]: https://crates.io/crates/tracing-core/0.1.34
[docs-badge]: https://docs.rs/tracing-core/badge.svg
[docs-url]: https://docs.rs/tracing-core/0.1.34
[docs-v0.2.x-badge]: https://img.shields.io/badge/docs-v0.2.x-blue
[docs-v0.2.x-url]: https://tracing.rs/tracing_core
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw

## Overview

[`tracing`] is a framework for instrumenting Rust programs to collect
structured, event-based diagnostic information. This crate defines the core
primitives of `tracing`.

The crate provides:

* [`span::Id`] identifies a span within the execution of a program.

* [`Event`] represents a single event within a trace.

* [`Subscriber`], the trait implemented to collect trace data.

* [`Metadata`] and [`Callsite`] provide information describing spans and
  events.

* [`Field`], [`FieldSet`], [`Value`], and [`ValueSet`] represent the
  structured data attached to spans and events.

* [`Dispatch`] allows spans and events to be dispatched to `Subscriber`s.

In addition, it defines the global callsite registry and per-thread current
dispatcher which other components of the tracing system rely on.

*Compiler support: [requires `rustc` 1.65+][msrv]*

[msrv]: #supported-rust-versions

## Usage

Application authors will typically not use this crate directly. Instead, they
will use the [`tracing`] crate, which provides a much more fully-featured
API. However, this crate's API will change very infrequently, so it may be used
when dependencies must be very stable.

`Subscriber` implementations may depend on `tracing-core` rather than `tracing`,
as the additional APIs provided by `tracing` are primarily useful for
instrumenting libraries and applications, and are generally not necessary for
`Subscriber` implementations.

###  Crate Feature Flags

The following crate feature flags are available:

* `std`: Depend on the Rust standard library (enabled by default).

  `no_std` users may disable this feature with `default-features = false`:

  ```toml
  [dependencies]
  tracing-core = { version = "0.1.34", default-features = false }
  ```

  **Note**:`tracing-core`'s `no_std` support requires `liballoc`.

[`tracing`]: ../tracing
[`span::Id`]: https://docs.rs/tracing-core/0.1.34/tracing_core/span/struct.Id.html
[`Event`]: https://docs.rs/tracing-core/0.1.34/tracing_core/event/struct.Event.html
[`Subscriber`]: https://docs.rs/tracing-core/0.1.34/tracing_core/subscriber/trait.Subscriber.html
[`Metadata`]: https://docs.rs/tracing-core/0.1.34/tracing_core/metadata/struct.Metadata.html
[`Callsite`]: https://docs.rs/tracing-core/0.1.34/tracing_core/callsite/trait.Callsite.html
[`Field`]: https://docs.rs/tracing-core/0.1.34/tracing_core/field/struct.Field.html
[`FieldSet`]: https://docs.rs/tracing-core/0.1.34/tracing_core/field/struct.FieldSet.html
[`Value`]: https://docs.rs/tracing-core/0.1.34/tracing_core/field/trait.Value.html
[`ValueSet`]: https://docs.rs/tracing-core/0.1.34/tracing_core/field/struct.ValueSet.html
[`Dispatch`]: https://docs.rs/tracing-core/0.1.34/tracing_core/dispatcher/struct.Dispatch.html

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.65. The current Tracing version is not guaranteed to build on Rust
versions earlier than the minimum supported version.

Tracing follows the same compiler support policies as the rest of the Tokio
project. The current stable Rust compiler and the three most recent minor
versions before it will always be supported. For example, if the current stable
compiler version is 1.69, the minimum supported version will not be increased
past 1.69, three minor versions prior. Increasing the minimum supported compiler
version is not considered a semver breaking change as long as doing so complies
with this policy.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
