![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/main/assets/splash.svg

# tracing-subscriber

Utilities for implementing and composing [`tracing`][tracing] subscribers.

[`tracing`] is a framework for instrumenting Rust programs to collect
scoped, structured, and async-aware diagnostics. The `Subscriber` trait
represents the functionality necessary to collect this trace data. This
crate contains tools for composing subscribers out of smaller units of
behaviour, and batteries-included implementations of common subscriber
functionality.

`tracing-subscriber` is intended for use by both `Subscriber` authors and
application authors using `tracing` to instrument their applications.

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (v0.2.x)][docs-v0.2.x-badge]][docs-v0.2.x-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]
![maintenance status][maint-badge]

[Documentation][docs-url] | [Chat][discord-url]

[tracing]: https://github.com/tokio-rs/tracing/tree/main/tracing
[tracing-fmt]: https://github.com/tokio-rs/tracing/tree/main/tracing-subscriber
[crates-badge]: https://img.shields.io/crates/v/tracing-subscriber.svg
[crates-url]: https://crates.io/crates/tracing-subscriber
[docs-badge]: https://docs.rs/tracing-subscriber/badge.svg
[docs-url]: https://docs.rs/tracing-subscriber/latest
[docs-v0.2.x-badge]: https://img.shields.io/badge/docs-v0.2.x-blue
[docs-v0.2.x-url]: https://tracing.rs/tracing_subscriber
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw
[maint-badge]: https://img.shields.io/badge/maintenance-experimental-blue.svg

*Compiler support: [requires `rustc` 1.65+][msrv]*

[msrv]: #supported-rust-versions

## Supported Rust Versions

Tracing is built against the latest stable release. The minimum supported
version is 1.65. The current Tracing version is not guaranteed to build on Rust
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
