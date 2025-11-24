![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/splash.svg

# tracing-log

[`log`] compatibility for [`tracing`].

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]
![maintenance status][maint-badge]

[Documentation][docs-url] | [Chat (discord)][discord-url]


[crates-badge]: https://img.shields.io/crates/v/tracing-log.svg
[crates-url]: https://crates.io/crates/tracing-log
[docs-badge]: https://docs.rs/tracing-log/badge.svg
[docs-url]: https://docs.rs/tracing-log
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tracing-rs.netlify.com/tracing_log
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw
[maint-badge]: https://img.shields.io/badge/maintenance-experimental-blue.svg

## Overview

[`tracing`] is a framework for instrumenting Rust programs with context-aware,
structured, event-based diagnostic information. This crate provides
compatibility layers for using `tracing` alongside the logging facade provided
by the [`log`] crate.

This crate provides:

- [`AsTrace`] and [`AsLog`] traits for converting between `tracing` and `log` types.
- [`LogTracer`], a [`log::Log`] implementation that consumes [`log::Record`]s
  and outputs them as [`tracing::Event`]s.

[`tracing`]: https://crates.io/crates/tracing
[`log`]: https://crates.io/crates/log
[`AsTrace`]: https://docs.rs/tracing-log/latest/tracing_log/trait.AsTrace.html
[`AsLog`]: https://docs.rs/tracing-log/latest/tracing_log/trait.AsLog.html
[`LogTracer`]: https://docs.rs/tracing-log/latest/tracing_log/struct.LogTracer.html
[`log::Log`]: https://docs.rs/log/latest/log/trait.Log.html
[`log::Record`]: https://docs.rs/log/latest/log/struct.Record.html
[`tracing::Subscriber`]: https://docs.rs/tracing/latest/tracing/trait.Subscriber.html
[`tracing::Event`]: https://docs.rs/tracing/latest/tracing/struct.Event.html

*Compiler support: [requires `rustc` 1.56+][msrv]*

[msrv]: #supported-rust-versions

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.56. The current Tracing version is not guaranteed to build on Rust
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
for inclusion in Tracing by you, shall be licensed as MIT, without any additional
terms or conditions.
